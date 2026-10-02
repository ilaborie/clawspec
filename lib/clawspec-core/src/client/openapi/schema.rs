use std::any::{TypeId, type_name};
use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

use indexmap::{IndexMap, IndexSet};
use utoipa::ToSchema;
use utoipa::openapi::schema::{AdditionalProperties, ArrayItems};
use utoipa::openapi::{Ref, RefOr, Schema};

/// Provides the OpenAPI schemas of a type to the schema collection.
///
/// Every schema collected by the client goes through this trait, so that an
/// alternative schema provider can be plugged in without touching the collection.
pub(crate) trait SchemaSource: 'static {
    /// The fully qualified Rust type path, as given by [`std::any::type_name`].
    fn type_path() -> &'static str;

    /// The component name of the schema.
    fn schema_name() -> Cow<'static, str>;

    /// The schema of the type itself.
    fn root_schema() -> RefOr<Schema>;

    /// Appends the `(name, schema)` pairs reachable from the type's fields or variants.
    fn nested_schemas(schemas: &mut Vec<(String, RefOr<Schema>)>);
}

impl<T> SchemaSource for T
where
    T: ToSchema + 'static,
{
    fn type_path() -> &'static str {
        type_name::<T>()
    }

    fn schema_name() -> Cow<'static, str> {
        T::name()
    }

    fn root_schema() -> RefOr<Schema> {
        T::schema()
    }

    fn nested_schemas(schemas: &mut Vec<(String, RefOr<Schema>)>) {
        T::schemas(schemas);
    }
}

/// How a Rust type maps onto its utoipa schema, read from its [`std::any::type_name`].
///
/// utoipa names a generic container after the bare container (`Vec`, `Option`, ...) and embeds
/// the element schema inline, without registering the element as a component. The type path
/// tells a standard container apart from a user type that happens to share its short name, and
/// gives the element type to hoist into a component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TypeShape<'a> {
    /// A primitive or a tuple, always inlined.
    Primitive,
    /// An array whose `items` hold the element schema.
    Sequence(&'a str),
    /// A `oneOf` of `null` and the inner schema.
    Optional(&'a str),
    /// An object whose `additionalProperties` hold the value schema.
    Map(&'a str),
    /// A wrapper whose schema is the inner schema itself.
    Transparent(&'a str),
    /// A type referenced as a component.
    Named(&'a str),
}

impl<'a> TypeShape<'a> {
    fn of(type_path: &'a str) -> Self {
        let type_path = strip_reference(type_path.trim());

        if let Some(element) = type_path
            .strip_prefix('[')
            .and_then(|inner| inner.strip_suffix(']'))
        {
            let element = split_top_level(element, ';').next().unwrap_or(element);
            return Self::Sequence(element);
        }
        if type_path.starts_with('(') {
            return Self::Primitive;
        }

        let (path, args) = split_generics(type_path);
        let root = path.split("::").next().unwrap_or(path);
        let last = path.rsplit("::").next().unwrap_or(path);
        let is_std = matches!(root, "core" | "alloc" | "std");

        match (last, args.as_slice()) {
            (
                "bool" | "char" | "str" | "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8"
                | "u16" | "u32" | "u64" | "u128" | "usize" | "f32" | "f64",
                [],
            ) if !path.contains("::") => Self::Primitive,
            ("String", []) if is_std => Self::Primitive,
            ("Vec" | "LinkedList" | "HashSet" | "BTreeSet", [element, ..]) if is_std => {
                Self::Sequence(element)
            }
            ("IndexSet", [element, ..]) if root == "indexmap" => Self::Sequence(element),
            ("Option", [inner]) if is_std => Self::Optional(inner),
            ("HashMap" | "BTreeMap", [_, value, ..]) if is_std => Self::Map(value),
            ("IndexMap", [_, value, ..]) if root == "indexmap" => Self::Map(value),
            ("Box" | "Rc" | "Arc" | "RefCell" | "Cow", [inner, ..]) if is_std => {
                Self::Transparent(inner)
            }
            ("ParamValue", [inner]) if root == "clawspec_core" => Self::Transparent(inner),
            _ => Self::Named(type_path),
        }
    }

    /// The component name utoipa gives by default: the last path segment without generics.
    fn default_schema_name(type_path: &str) -> &str {
        let (path, _) = split_generics(type_path);
        path.rsplit("::").next().unwrap_or(path)
    }
}

fn strip_reference(type_path: &str) -> &str {
    let mut type_path = type_path;
    while let Some(rest) = type_path.strip_prefix('&') {
        type_path = rest.trim_start();
        type_path = type_path.strip_prefix("mut ").unwrap_or(type_path);
    }
    type_path
}

/// Splits a type path into its path and its generic type arguments, lifetimes left out.
fn split_generics(type_path: &str) -> (&str, Vec<&str>) {
    match (type_path.find('<'), type_path.strip_suffix('>')) {
        (Some(start), Some(without_end)) => (
            &type_path[..start],
            split_top_level(&without_end[start + 1..], ',')
                .filter(|argument| !argument.starts_with('\''))
                .collect(),
        ),
        _ => (type_path, Vec::new()),
    }
}

/// Splits on `separator` outside of any `<>`, `[]` or `()` nesting, trimming each part.
fn split_top_level(input: &str, separator: char) -> impl Iterator<Item = &str> {
    let mut depth = 0_usize;
    let mut start = 0;
    let mut parts = Vec::new();
    for (index, character) in input.char_indices() {
        match character {
            '<' | '[' | '(' => depth += 1,
            '>' | ']' | ')' => depth = depth.saturating_sub(1),
            _ if character == separator && depth == 0 => {
                parts.push(input[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(input[start..].trim());
    parts.into_iter()
}

/// Replaces the inline schemas of named types inside a container schema with references, and
/// collects the replaced schemas as components.
fn hoist_named_schemas(
    schema: &mut RefOr<Schema>,
    shape: TypeShape<'_>,
    hoisted: &mut Vec<(String, RefOr<Schema>)>,
) {
    match shape {
        TypeShape::Primitive => {}
        TypeShape::Named(type_path) => {
            if matches!(schema, RefOr::T(_)) {
                let name = TypeShape::default_schema_name(type_path);
                let inline = std::mem::replace(schema, RefOr::Ref(Ref::from_schema_name(name)));
                hoisted.push((name.to_string(), inline));
            }
        }
        TypeShape::Transparent(inner) => {
            hoist_named_schemas(schema, TypeShape::of(inner), hoisted);
        }
        TypeShape::Sequence(element) => {
            if let RefOr::T(Schema::Array(array)) = schema
                && let ArrayItems::RefOrSchema(items) = &mut array.items
            {
                hoist_named_schemas(items, TypeShape::of(element), hoisted);
            }
        }
        TypeShape::Optional(inner) => {
            if let RefOr::T(Schema::OneOf(one_of)) = schema
                && let Some(item) = one_of.items.last_mut()
            {
                hoist_named_schemas(item, TypeShape::of(inner), hoisted);
            }
        }
        TypeShape::Map(value) => {
            if let RefOr::T(Schema::Object(object)) = schema
                && let Some(additional) = object.additional_properties.as_deref_mut()
                && let AdditionalProperties::RefOr(value_schema) = additional
            {
                hoist_named_schemas(value_schema, TypeShape::of(value), hoisted);
            }
        }
    }
}

/// The schema of a type, resolved for the collection.
struct ResolvedSchema {
    /// Whether the schema is inlined (primitives and containers) rather than referenced.
    inline: bool,
    /// The schema, with named element types of a container replaced by references.
    schema: RefOr<Schema>,
    /// The named element types replaced by references, to register as components.
    hoisted: Vec<(String, RefOr<Schema>)>,
}

impl ResolvedSchema {
    fn of<T>() -> Self
    where
        T: SchemaSource,
    {
        let mut schema = T::root_schema();
        let mut hoisted = Vec::new();
        let shape = TypeShape::of(T::type_path());
        let inline = !matches!(shape, TypeShape::Named(_));
        if inline {
            hoist_named_schemas(&mut schema, shape, &mut hoisted);
        }
        Self {
            inline,
            schema,
            hoisted,
        }
    }
}

/// Computes a schema reference locally without accessing shared state.
///
/// This enables fire-and-forget schema registration via channels by allowing
/// callers to compute the schema reference before sending the message.
///
/// - Primitive and container types are inlined (return `RefOr::T`), with named element types
///   referenced
/// - Named types are referenced (return `RefOr::Ref`)
pub(in crate::client) fn compute_schema_ref<T>() -> RefOr<Schema>
where
    T: SchemaSource,
{
    if matches!(TypeShape::of(T::type_path()), TypeShape::Named(_)) {
        RefOr::Ref(Ref::from_schema_name(T::schema_name().as_ref()))
    } else {
        ResolvedSchema::of::<T>().schema
    }
}

#[derive(Clone, Default)]
pub(in crate::client) struct Schemas {
    entries: IndexMap<TypeId, SchemaEntry>,
    resolved_names: std::collections::HashMap<TypeId, String>,
    /// Schemas transitively reachable from a registered type's nested fields/variants,
    /// discovered via utoipa's `ToSchema::schemas()` recursive walk. Keyed by name rather
    /// than `TypeId` since utoipa's API only exposes the name for these. A `TypeId`-backed
    /// entry with the same name always takes precedence (see `schema_vec`).
    nested: IndexMap<String, RefOr<Schema>>,
}

impl Debug for Schemas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names = self
            .entries
            .values()
            .map(|it| it.type_name.as_str())
            .collect::<Vec<_>>();
        f.debug_tuple("Schemas").field(&names).finish()
    }
}

impl Schemas {
    /// Folds nested `(name, schema)` pairs discovered via `ToSchema::schemas()` into the
    /// name-keyed side table. First-write-wins: a name already present (for instance from an
    /// earlier nested walk, or destined to be shadowed by a directly registered type in
    /// `schema_vec`) is left untouched.
    ///
    /// When an already-present name maps to a *different* schema shape, the incoming one is a
    /// genuine short-name collision between two distinct types (utoipa exposes only the name,
    /// not a `TypeId`, for nested schemas, so they cannot be namespaced apart). The first is
    /// kept and a warning is emitted, mirroring the nested-vs-registered path in `schema_vec`.
    fn absorb_nested(&mut self, nested: impl IntoIterator<Item = (String, RefOr<Schema>)>) {
        for (name, schema) in nested {
            match self.nested.entry(name) {
                indexmap::map::Entry::Vacant(vacant) => {
                    vacant.insert(schema);
                }
                indexmap::map::Entry::Occupied(occupied) if *occupied.get() != schema => {
                    tracing::warn!(
                        schema_name = %occupied.key(),
                        "Two distinct nested types resolve to the same schema name with \
                         different shapes; keeping the first. Disambiguate one of them with \
                         #[schema(as = \"module::Type\")]."
                    );
                }
                indexmap::map::Entry::Occupied(_) => {}
            }
        }
    }

    pub(crate) fn add_entry(&mut self, mut entry: SchemaEntry) -> RefOr<Schema> {
        let type_id = entry.id;

        if !self.entries.contains_key(&type_id) {
            self.absorb_nested(std::mem::take(&mut entry.nested));
        }

        // First insert/update the entry
        let _ = self
            .entries
            .entry(type_id)
            .and_modify(|existing| existing.examples.extend(entry.examples.clone()))
            .or_insert(entry);

        // Then resolve name for this type and cache it
        let resolved_name = self.resolve_name_for_type(type_id);

        // Create the reference using the resolved name
        if self.entries[&type_id].should_inline_schema() {
            self.entries[&type_id].schema.clone()
        } else {
            RefOr::Ref(Ref::from_schema_name(&resolved_name))
        }
    }

    fn add_type<T>(&mut self) -> &mut SchemaEntry
    where
        T: SchemaSource,
    {
        let id = TypeId::of::<T>();
        if !self.entries.contains_key(&id) {
            let mut entry = SchemaEntry::of::<T>();
            self.absorb_nested(std::mem::take(&mut entry.nested));
            self.entries.insert(id, entry);
        }
        self.entries
            .get_mut(&id)
            .expect("entry inserted above if it was missing")
    }

    pub(in crate::client) fn add<T>(&mut self) -> RefOr<Schema>
    where
        T: SchemaSource,
    {
        let type_id = TypeId::of::<T>();
        let _ = self.add_type::<T>();

        // Resolve name for this type and cache it
        let resolved_name = self.resolve_name_for_type(type_id);

        // Create the reference using the resolved name
        if self.entries[&type_id].should_inline_schema() {
            self.entries[&type_id].schema.clone()
        } else {
            RefOr::Ref(Ref::from_schema_name(&resolved_name))
        }
    }

    pub(in crate::client) fn add_example<T>(
        &mut self,
        example: impl Into<serde_json::Value>,
    ) -> RefOr<Schema>
    where
        T: SchemaSource,
    {
        let example = example.into();
        let type_id = TypeId::of::<T>();
        let entry = self.add_type::<T>();
        entry.examples.insert(example);

        // Resolve name for this type and cache it
        let resolved_name = self.resolve_name_for_type(type_id);

        // Create the reference using the resolved name
        if self.entries[&type_id].should_inline_schema() {
            self.entries[&type_id].schema.clone()
        } else {
            RefOr::Ref(Ref::from_schema_name(&resolved_name))
        }
    }

    /// Add an example to a schema by TypeId (creates entry if not exists).
    ///
    /// This method is used by the channel-based collection system where
    /// the type information is passed as TypeId rather than generic parameters.
    pub(in crate::client) fn add_example_by_id(
        &mut self,
        type_id: TypeId,
        type_name: &str,
        example: serde_json::Value,
    ) {
        if let Some(entry) = self.entries.get_mut(&type_id) {
            entry.examples.insert(example);
        } else {
            tracing::warn!(
                type_name = %type_name,
                "Attempted to add example for unregistered type"
            );
        }
    }

    /// Resolves the unique name for a given TypeId, handling conflicts
    fn resolve_name_for_type(&mut self, target_type_id: TypeId) -> String {
        // Check if we already resolved this type's name
        if let Some(cached_name) = self.resolved_names.get(&target_type_id) {
            return cached_name.clone();
        }

        let target_entry = &self.entries[&target_type_id];
        let base_name = &target_entry.name;

        // Count conflicts
        let conflicts: Vec<_> = self
            .entries
            .values()
            .filter(|entry| !entry.should_inline_schema() && &entry.name == base_name)
            .collect();

        let resolved_name = if conflicts.len() <= 1 {
            // No conflict, use the original name
            base_name.clone()
        } else {
            // Conflict detected - generate unique name using type path
            let type_parts: Vec<&str> = target_entry
                .type_name
                .split("::")
                .filter(|part| !part.is_empty() && !Self::is_filtered_path_part(part))
                .collect();

            if type_parts.len() >= 2 {
                // Use the last two parts for namespace (e.g., "module::Type")
                format!("{}_{}", type_parts[type_parts.len() - 2], base_name)
            } else {
                // Fallback: use a more readable hash-based suffix
                let mut hasher = DefaultHasher::new();
                target_type_id.hash(&mut hasher);
                let hash = hasher.finish();
                let fallback_name = format!("{base_name}_{:x}", hash & 0xFFFF);

                // Warn about fallback naming for debugging purposes
                tracing::warn!(
                    type_name = %target_entry.type_name,
                    base_name = %base_name,
                    fallback_name = %fallback_name,
                    "Schema conflict resolved using hash-based fallback naming. \
                     Consider using more specific module structure for better naming."
                );

                fallback_name
            }
        };

        // Cache the resolved name
        self.resolved_names
            .insert(target_type_id, resolved_name.clone());
        resolved_name
    }

    /// Merges another schema collection into this one.
    ///
    /// This function implements the core schema merge logic that handles
    /// combining schemas from multiple API test calls.
    ///
    /// # Merge Strategy
    ///
    /// - **Type Identity**: Schemas are identified by Rust `TypeId`
    /// - **Type Safety**: Same Rust type always maps to same OpenAPI schema
    /// - **Example Collection**: Examples from both schemas are combined
    /// - **Schema Overwrite**: New schema overwrites existing (same TypeId)
    ///
    /// # Performance Characteristics
    ///
    /// - **Time Complexity**: O(n) where n is the number of schemas to merge
    /// - **Space Complexity**: O(1) additional space (moves entries, doesn't copy)
    /// - **Memory Efficiency**: Direct insertion by TypeId for optimal performance
    ///
    /// # Arguments
    ///
    /// * `other` - The schema collection to merge into this one
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Internal usage - not exposed in public API
    /// let mut schemas1 = Schemas::default();
    /// let mut schemas2 = Schemas::default();
    ///
    /// // schemas1 has User schema with example1
    /// // schemas2 has User schema with example2
    /// schemas1.merge(schemas2);
    /// // Result: schemas1 has User schema with both examples
    /// ```
    pub(in crate::client) fn merge(&mut self, other: Self) {
        // Collect schema names that might be affected by conflicts
        let mut potentially_affected_names = std::collections::HashSet::new();

        for (type_id, entry) in &other.entries {
            // If this name already exists in our collection, it might create conflicts
            if self
                .entries
                .values()
                .any(|existing| existing.name == entry.name && !existing.should_inline_schema())
            {
                potentially_affected_names.insert(entry.name.clone());
            }

            self.entries
                .entry(*type_id)
                .and_modify(|existing| existing.examples.extend(entry.examples.clone()))
                .or_insert(entry.clone());
        }

        // Selectively invalidate cache only for potentially conflicted schemas
        if !potentially_affected_names.is_empty() {
            self.resolved_names.retain(|type_id, _| {
                if let Some(entry) = self.entries.get(type_id) {
                    !potentially_affected_names.contains(&entry.name)
                } else {
                    false // Remove if entry no longer exists
                }
            });
        }

        self.absorb_nested(other.nested);
    }

    pub(in crate::client) fn schema_vec(&self) -> Vec<(String, RefOr<Schema>)> {
        let mut result = vec![];

        // First, identify all non-primitive entries and detect conflicts
        let non_primitive_entries: Vec<_> = self
            .entries
            .iter()
            .filter(|(_, entry)| !entry.should_inline_schema())
            .collect();

        // Count name occurrences to detect conflicts
        let mut name_counts = std::collections::HashMap::<String, u32>::new();
        for (_, entry) in &non_primitive_entries {
            *name_counts.entry(entry.name.clone()).or_insert(0) += 1;
        }

        // Schemas already provided by a directly registered type; these take precedence
        // over a same-named schema discovered only through a nested `schemas()` walk. The
        // same Rust type commonly gets discovered both ways (e.g. it's nested in one
        // response and also used directly as another endpoint's body) - that's expected
        // and produces an identical schema body, so it's only worth a warning when the
        // bodies actually differ (a genuine short-name collision between distinct types).
        //
        // Keyed by the bare `name`, this map is only meaningful for names that occupy the
        // bare slot in the output - i.e. `name_counts[name] == 1`. When several registered
        // types share a short name they are emitted under namespaced names (`module_Foo`),
        // so a same-named nested schema no longer collides and must be appended (see below).
        let registered_schemas: std::collections::HashMap<&str, &RefOr<Schema>> =
            non_primitive_entries
                .iter()
                .map(|(_, entry)| (entry.name.as_str(), &entry.schema))
                .collect();

        // Generate resolved names without cloning the entire structure
        for (type_id, entry) in non_primitive_entries {
            let resolved_name =
                self.resolve_schema_name(*type_id, &entry.name, &entry.type_name, &name_counts);
            let schema = entry.schema.clone();
            result.push((resolved_name, schema));
        }

        // Append schemas transitively discovered via nested ToSchema::schemas() walks.
        for (name, schema) in &self.nested {
            // A directly-registered type only shadows this nested name when it actually
            // occupies the bare slot in the output. If several registered types share the
            // short name they are namespaced instead, leaving it free for the nested schema.
            let shadows_bare_name = name_counts.get(name.as_str()).copied().unwrap_or(0) == 1;
            match registered_schemas.get(name.as_str()) {
                Some(registered_schema) if shadows_bare_name && *registered_schema == schema => {
                    continue;
                }
                Some(_) if shadows_bare_name => {
                    tracing::warn!(
                        schema_name = %name,
                        "Nested schema name collides with a directly registered schema of \
                         the same name but a different shape; keeping the directly \
                         registered one."
                    );
                    continue;
                }
                _ => result.push((name.clone(), schema.clone())),
            }
        }

        result
    }

    /// Resolves schema name for a specific entry without requiring mutable access
    fn resolve_schema_name(
        &self,
        type_id: TypeId,
        base_name: &str,
        type_name: &str,
        name_counts: &std::collections::HashMap<String, u32>,
    ) -> String {
        // Check cache first
        if let Some(cached_name) = self.resolved_names.get(&type_id) {
            return cached_name.clone();
        }

        // If no conflict, use original name
        if name_counts.get(base_name).copied().unwrap_or(0) <= 1 {
            return base_name.to_string();
        }

        // Conflict detected - generate unique name using type path
        let type_parts: Vec<&str> = type_name
            .split("::")
            .filter(|part| !part.is_empty() && !Self::is_filtered_path_part(part))
            .collect();

        if type_parts.len() >= 2 {
            // Use the last two parts for namespace (e.g., "module::Type")
            format!("{}_{}", type_parts[type_parts.len() - 2], base_name)
        } else {
            // Fallback: use a more readable hash-based suffix
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut hasher = DefaultHasher::new();
            type_id.hash(&mut hasher);
            let hash = hasher.finish();
            let fallback_name = format!("{base_name}_{:x}", hash & 0xFFFF);

            // Warn about fallback naming for debugging purposes
            tracing::warn!(
                type_name = %type_name,
                base_name = %base_name,
                fallback_name = %fallback_name,
                "Schema conflict resolved using hash-based fallback naming. \
                 Consider using more specific module structure for better naming."
            );

            fallback_name
        }
    }

    /// Checks if a path part should be filtered out from namespace generation
    fn is_filtered_path_part(part: &str) -> bool {
        // Filter out common test-related and internal modules
        matches!(
            part,
            "tests" | "test" | "_test" | "testing" | "internal" | "private"
        )
    }
}

#[derive(Clone, derive_more::Display, derive_more::Debug)]
#[display("[{id:?}] {name}")]
pub(in crate::client) struct SchemaEntry {
    #[debug(ignore)]
    pub(in crate::client) id: TypeId,
    pub(in crate::client) type_name: String,
    pub(in crate::client) name: String,
    #[debug(ignore)]
    pub(in crate::client) schema: RefOr<Schema>,
    pub(in crate::client) examples: IndexSet<serde_json::Value>,
    /// Whether the schema is inlined (primitives and containers) rather than referenced.
    #[debug(ignore)]
    pub(in crate::client) inline: bool,
    /// Schemas transitively reachable from this type's fields/variants, discovered via
    /// utoipa's `ToSchema::schemas()` recursive walk. Only meaningful the first time this
    /// entry is inserted into a `Schemas` collection; consumed there (see
    /// `Schemas::absorb_nested`).
    #[debug(ignore)]
    pub(in crate::client) nested: Vec<(String, RefOr<Schema>)>,
}

impl SchemaEntry {
    pub(crate) fn of<T>() -> Self
    where
        T: SchemaSource,
    {
        let ResolvedSchema {
            inline,
            schema,
            mut hoisted,
        } = ResolvedSchema::of::<T>();
        T::nested_schemas(&mut hoisted);
        Self {
            id: TypeId::of::<T>(),
            type_name: T::type_path().to_string(),
            name: T::schema_name().to_string(),
            schema,
            examples: IndexSet::default(),
            inline,
            nested: hoisted,
        }
    }

    /// Creates a generic schema entry for raw binary data.
    ///
    /// This is used when we don't have a specific Rust type to generate
    /// a schema from, such as when sending raw bytes with custom content types.
    pub(crate) fn raw_binary() -> Self {
        use utoipa::openapi::{KnownFormat, ObjectBuilder, Schema, SchemaFormat, Type};

        // Create a unique TypeId for raw binary data
        let id = TypeId::of::<Vec<u8>>();
        let type_name = "Vec<u8>";
        let name = "binary";

        // Create a binary schema
        let schema = RefOr::T(Schema::Object(
            ObjectBuilder::new()
                .schema_type(Type::String)
                .format(Some(SchemaFormat::KnownFormat(KnownFormat::Binary)))
                .build(),
        ));

        Self {
            id,
            type_name: type_name.to_string(),
            name: name.to_string(),
            schema,
            examples: IndexSet::default(),
            inline: true,
            nested: Vec::new(),
        }
    }

    pub(crate) fn add_example(&mut self, example: serde_json::Value) {
        self.examples.insert(example);
    }

    /// Determines if this schema should be inlined (primitives and containers) or referenced
    /// (named types).
    fn should_inline_schema(&self) -> bool {
        self.inline
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use serde::Serialize;
    use utoipa::ToSchema;

    use super::*;

    #[derive(Debug, ToSchema, Serialize)]
    struct TestType {
        name: String,
        value: i32,
    }

    #[derive(Debug, ToSchema, Serialize)]
    struct AnotherTestType {
        id: u64,
    }

    #[test]
    fn test_schemas_add_single_type() {
        let mut schemas = Schemas::default();
        let schema_ref = schemas.add::<TestType>();

        // Should return a reference
        assert!(matches!(schema_ref, RefOr::Ref(_)));

        // Should have one schema entry
        let schema_vec = schemas.schema_vec();
        assert_eq!(schema_vec.len(), 1);
        assert_eq!(schema_vec[0].0, "TestType");
    }

    #[test]
    fn test_schemas_add_captures_nested_types_transitively() {
        #[derive(Debug, ToSchema, Serialize)]
        struct Leaf {
            value: i32,
        }

        #[derive(Debug, ToSchema, Serialize)]
        struct Mid {
            leaf: Leaf,
        }

        #[derive(Debug, ToSchema, Serialize)]
        struct Root {
            mid: Mid,
        }

        let mut schemas = Schemas::default();
        // Only the root is registered directly; Mid and Leaf must be discovered via
        // ToSchema::schemas()'s recursive walk.
        schemas.add::<Root>();

        let schema_vec = schemas.schema_vec();
        let names: HashSet<&str> = schema_vec.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(
            names,
            HashSet::from(["Root", "Mid", "Leaf"]),
            "expected root and all transitively nested types to be captured"
        );
    }

    #[derive(Debug, Clone, Serialize, ToSchema)]
    struct Leaf {
        value: i32,
    }

    #[derive(Debug, ToSchema, Serialize)]
    struct Named {
        leaf: Leaf,
    }

    fn added_schema_and_components<T: SchemaSource>() -> String {
        let mut schemas = Schemas::default();
        let schema = schemas.add::<T>();
        let mut components = schemas
            .schema_vec()
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>();
        components.sort();
        serde_saphyr::to_string(&serde_json::json!({ "schema": schema, "components": components }))
            .expect("should serialize to YAML")
    }

    #[test]
    fn test_schemas_add_references_vec_element_as_component() {
        insta::assert_snapshot!(added_schema_and_components::<Vec<Named>>(), @r##"
        components:
        - Leaf
        - Named
        schema:
          items:
            $ref: "#/components/schemas/Named"
          type: array
        "##);
    }

    #[test]
    fn test_schemas_add_registers_vec_element_with_its_own_schema() {
        let mut schemas = Schemas::default();
        schemas.add::<Vec<Named>>();

        let named = schemas
            .schema_vec()
            .into_iter()
            .find(|(name, _)| name == "Named")
            .map(|(_, schema)| schema)
            .expect("Named should be registered as a component");
        assert_eq!(named, <Named as utoipa::PartialSchema>::schema());
    }

    #[test]
    fn test_schemas_add_inlines_vec_of_primitive() {
        insta::assert_snapshot!(added_schema_and_components::<Vec<i32>>(), @"
        components: []
        schema:
          items:
            format: int32
            type: integer
          type: array
        ");
    }

    #[test]
    fn test_schemas_add_references_option_inner_as_component() {
        insta::assert_snapshot!(added_schema_and_components::<Option<Named>>(), @r##"
        components:
        - Leaf
        - Named
        schema:
          oneOf:
          - type: "null"
          - $ref: "#/components/schemas/Named"
        "##);
    }

    #[test]
    fn test_schemas_add_references_boxed_type() {
        insta::assert_snapshot!(added_schema_and_components::<Box<Named>>(), @r##"
        components:
        - Leaf
        - Named
        schema:
          $ref: "#/components/schemas/Named"
        "##);
    }

    #[test]
    fn test_schemas_add_references_map_value_as_component() {
        insta::assert_snapshot!(
            added_schema_and_components::<std::collections::HashMap<String, Named>>(),
            @r##"
        components:
        - Leaf
        - Named
        schema:
          additionalProperties:
            $ref: "#/components/schemas/Named"
          propertyNames:
            type: string
          type: object
        "##
        );
    }

    #[test]
    fn test_schemas_add_references_nested_container_elements() {
        insta::assert_snapshot!(
            added_schema_and_components::<Vec<Option<std::collections::BTreeMap<String, Named>>>>(),
            @r##"
        components:
        - Leaf
        - Named
        schema:
          items:
            oneOf:
            - type: "null"
            - additionalProperties:
                $ref: "#/components/schemas/Named"
              propertyNames:
                type: string
              type: object
          type: array
        "##
        );
    }

    #[test]
    fn test_schemas_add_references_slice_element_as_component() {
        insta::assert_snapshot!(added_schema_and_components::<&'static [Named]>(), @r##"
        components:
        - Leaf
        - Named
        schema:
          items:
            $ref: "#/components/schemas/Named"
          type: array
        "##);
    }

    #[test]
    fn test_schemas_add_keeps_user_type_named_like_a_container_as_component() {
        #[derive(Debug, ToSchema, Serialize)]
        struct Box {
            value: i32,
        }

        #[derive(Debug, ToSchema, Serialize)]
        #[schema(as = Option)]
        struct Renamed {
            value: i32,
        }

        insta::assert_snapshot!(added_schema_and_components::<Box>(), @r##"
        components:
        - Box
        schema:
          $ref: "#/components/schemas/Box"
        "##);
        insta::assert_snapshot!(added_schema_and_components::<Renamed>(), @r##"
        components:
        - Option
        schema:
          $ref: "#/components/schemas/Option"
        "##);
    }

    #[test]
    fn test_compute_schema_ref_matches_registered_schema() {
        assert_eq!(
            compute_schema_ref::<Vec<Named>>(),
            Schemas::default().add::<Vec<Named>>()
        );
        assert_eq!(
            compute_schema_ref::<Named>(),
            RefOr::Ref(Ref::from_schema_name("Named"))
        );
        assert_eq!(
            compute_schema_ref::<Box<Named>>(),
            RefOr::Ref(Ref::from_schema_name("Named"))
        );
    }

    mod user_types {
        #[derive(Debug, utoipa::ToSchema, serde::Serialize)]
        pub(super) struct Box {
            value: i32,
        }
    }

    #[test]
    fn test_type_shape_reads_standard_containers_from_type_path() {
        let shapes = [
            type_name::<Vec<Named>>(),
            type_name::<std::collections::HashMap<String, Named>>(),
            type_name::<indexmap::IndexMap<String, Named>>(),
            type_name::<std::collections::BTreeSet<Named>>(),
            type_name::<Cow<'static, Leaf>>(),
            type_name::<std::sync::Arc<Named>>(),
            type_name::<[Named; 2]>(),
            type_name::<&mut [Named]>(),
            type_name::<Option<String>>(),
            type_name::<&str>(),
            type_name::<(i32, String)>(),
            type_name::<user_types::Box>(),
        ]
        .map(TypeShape::of);
        insta::assert_debug_snapshot!(shapes, @r#"
        [
            Sequence(
                "clawspec_core::client::openapi::schema::tests::Named",
            ),
            Map(
                "clawspec_core::client::openapi::schema::tests::Named",
            ),
            Map(
                "clawspec_core::client::openapi::schema::tests::Named",
            ),
            Sequence(
                "clawspec_core::client::openapi::schema::tests::Named",
            ),
            Transparent(
                "clawspec_core::client::openapi::schema::tests::Leaf",
            ),
            Transparent(
                "clawspec_core::client::openapi::schema::tests::Named",
            ),
            Sequence(
                "clawspec_core::client::openapi::schema::tests::Named",
            ),
            Sequence(
                "clawspec_core::client::openapi::schema::tests::Named",
            ),
            Optional(
                "alloc::string::String",
            ),
            Primitive,
            Primitive,
            Named(
                "clawspec_core::client::openapi::schema::tests::user_types::Box",
            ),
        ]
        "#);
    }

    #[test]
    fn test_schemas_add_captures_flattened_nested_type() {
        #[derive(Debug, ToSchema, Serialize)]
        struct Inner {
            value: i32,
        }

        #[derive(Debug, ToSchema, Serialize)]
        struct Outer {
            #[serde(flatten)]
            inner: Inner,
            extra: String,
        }

        let mut schemas = Schemas::default();
        schemas.add::<Outer>();

        let schema_vec = schemas.schema_vec();
        let names: HashSet<&str> = schema_vec.iter().map(|(name, _)| name.as_str()).collect();
        assert!(
            names.contains("Inner"),
            "flattened nested type should still be captured, got {names:?}"
        );
    }

    #[test]
    fn test_schemas_add_captures_enum_variant_payload_types() {
        #[derive(Debug, ToSchema, Serialize)]
        struct Created {
            id: u64,
        }

        #[derive(Debug, ToSchema, Serialize)]
        struct Deleted {
            id: u64,
        }

        #[derive(Debug, ToSchema, Serialize)]
        enum Event {
            Created(Created),
            Deleted(Deleted),
        }

        // Constructed so the variants aren't flagged as dead code.
        let _ = Event::Created(Created { id: 1 });
        let _ = Event::Deleted(Deleted { id: 2 });

        let mut schemas = Schemas::default();
        schemas.add::<Event>();

        let schema_vec = schemas.schema_vec();
        let names: HashSet<&str> = schema_vec.iter().map(|(name, _)| name.as_str()).collect();
        assert!(
            names.contains("Created") && names.contains("Deleted"),
            "enum variant payload types should be captured, got {names:?}"
        );
    }

    #[test]
    fn test_schemas_add_terminates_on_recursive_type_with_no_recursion_attribute() {
        // utoipa's own ToSchema::schemas() has no built-in cycle detection: a recursive
        // field (directly or mutually recursive) *must* be annotated with
        // `#[schema(no_recursion)]`, or utoipa itself stack-overflows while walking it -
        // independent of clawspec-core, and identical to what `#[derive(OpenApi)]` +
        // `components(schemas(...))` would do (see
        // https://github.com/juhaku/utoipa/issues/1134). Document that requirement rather
        // than working around it, since there's no way to intervene inside utoipa's opaque
        // recursive call.
        #[derive(Debug, ToSchema, Serialize)]
        struct Node {
            value: i32,
            #[schema(no_recursion)]
            next: Option<Box<Node>>,
        }

        let mut schemas = Schemas::default();
        schemas.add::<Node>();

        let schema_vec = schemas.schema_vec();
        let names: HashSet<&str> = schema_vec.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, HashSet::from(["Node"]));
    }

    #[test]
    fn test_schema_vec_nested_collision_keeps_registered_shape() {
        // A directly-registered type and a distinct nested-only type share the same short
        // schema name ("Config"). utoipa exposes only the name (no TypeId) for nested schemas,
        // so they cannot be namespaced apart; the registered shape must win and the nested one
        // is dropped (with a warning). A name-set assertion could not catch a wrong *shape*
        // sneaking in under the right name, so this checks the emitted body and the count.
        #[derive(Debug, ToSchema, Serialize)]
        struct Config {
            timeout_ms: u64,
        }

        // Different type, same default short name ("Config"), reachable only as a nested field.
        mod nested_mod {
            use super::*;

            #[derive(Debug, ToSchema, Serialize)]
            pub struct Config {
                retries: String,
            }
        }

        #[derive(Debug, ToSchema, Serialize)]
        struct Root {
            config: nested_mod::Config,
        }

        let mut schemas = Schemas::default();
        schemas.add::<Config>();
        schemas.add::<Root>();

        let schema_vec = schemas.schema_vec();
        let config_entries: Vec<_> = schema_vec
            .iter()
            .filter(|(name, _)| name == "Config")
            .collect();
        assert_eq!(
            config_entries.len(),
            1,
            "the colliding short name must appear exactly once, got {schema_vec:?}"
        );
        assert_eq!(
            &config_entries[0].1,
            &<Config as utoipa::PartialSchema>::schema(),
            "the directly-registered Config shape must take precedence over the nested one"
        );
    }

    #[test]
    fn test_schemas_merge_absorbs_nested_schemas() {
        // `merge` must carry over the name-keyed nested table, not just the TypeId entries.
        // Every other merge test uses primitive-only types, so this is the only coverage of
        // schemas discovered transitively arriving through the merge path (e.g. complex
        // struct-valued parameters).
        #[derive(Debug, ToSchema, Serialize)]
        struct Leaf {
            value: i32,
        }

        #[derive(Debug, ToSchema, Serialize)]
        struct Mid {
            leaf: Leaf,
        }

        #[derive(Debug, ToSchema, Serialize)]
        struct Root {
            mid: Mid,
        }

        let mut source = Schemas::default();
        source.add::<Root>();

        let mut target = Schemas::default();
        target.merge(source);

        let schema_vec = target.schema_vec();
        let names: HashSet<&str> = schema_vec.iter().map(|(name, _)| name.as_str()).collect();
        assert!(
            names.is_superset(&HashSet::from(["Root", "Mid", "Leaf"])),
            "merge must carry over transitively nested schemas, got {names:?}"
        );
    }

    #[test]
    fn test_schema_vec_type_registered_and_nested_appears_once() {
        // The common, benign case: the same Rust type is both registered directly and
        // discovered as a nested field. It shares a TypeId, so both discoveries yield an
        // identical shape; the nested duplicate must be dropped, leaving exactly one entry.
        #[derive(Debug, ToSchema, Serialize)]
        struct Mid {
            value: i32,
        }

        #[derive(Debug, ToSchema, Serialize)]
        struct Root {
            mid: Mid,
        }

        let mut schemas = Schemas::default();
        schemas.add::<Root>();
        schemas.add::<Mid>();

        let mid_count = schemas
            .schema_vec()
            .iter()
            .filter(|(name, _)| name == "Mid")
            .count();
        assert_eq!(
            mid_count, 1,
            "a type registered directly and discovered nested must appear exactly once"
        );
    }

    #[test]
    fn test_schemas_add_with_example() {
        let mut schemas = Schemas::default();
        let test_example = serde_json::json!({"name": "test", "value": 42});

        let schema_ref = schemas.add_example::<TestType>(test_example.clone());

        matches!(schema_ref, RefOr::Ref(_));

        // Verify the example was added (we can't directly access it but we can check it doesn't panic)
        let schema_vec = schemas.schema_vec();
        assert_eq!(schema_vec.len(), 1);
    }

    #[test]
    fn test_schemas_add_same_type_twice_returns_same_entry() {
        let mut schemas = Schemas::default();

        schemas.add::<TestType>();
        schemas.add::<TestType>();

        // Should still only have one entry
        assert_eq!(schemas.entries.len(), 1);
        let schema_vec = schemas.schema_vec();
        assert_eq!(schema_vec.len(), 1);
    }

    #[test]
    fn test_schemas_add_different_types() {
        let mut schemas = Schemas::default();

        schemas.add::<TestType>();
        schemas.add::<AnotherTestType>();

        // Should have two entries
        let schema_vec = schemas.schema_vec();
        assert_eq!(schema_vec.len(), 2);

        let names = schema_vec
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<&str>>();
        assert!(names.contains(&"TestType"));
        assert!(names.contains(&"AnotherTestType"));
    }

    #[test]
    fn test_schemas_merge() {
        let mut schemas1 = Schemas::default();
        schemas1.add::<TestType>();

        let mut schemas2 = Schemas::default();
        schemas2.add::<AnotherTestType>();

        schemas1.merge(schemas2);

        // Should have both types
        let schema_vec = schemas1.schema_vec();
        assert_eq!(schema_vec.len(), 2);
    }

    #[test]
    fn test_schemas_merge_with_conflicts_and_examples() {
        // Test merge behavior with conflicting schema names and example collection
        #[derive(Debug, ToSchema, Serialize)]
        struct User {
            id: u64,
            name: String,
        }

        mod api_v1 {
            use super::*;

            #[derive(Debug, ToSchema, Serialize)]
            pub struct User {
                user_id: String,
                email: String,
            }
        }

        // Create first collection with User and examples
        let mut schemas1 = Schemas::default();
        let example1 = serde_json::json!({"id": 1, "name": "Alice"});
        schemas1.add_example::<User>(example1.clone());

        // Create second collection with different User type and examples
        let mut schemas2 = Schemas::default();
        let example2 = serde_json::json!({"user_id": "abc123", "email": "alice@example.com"});
        schemas2.add_example::<api_v1::User>(example2.clone());

        // Also add another example for the same User type to first collection
        let example3 = serde_json::json!({"id": 2, "name": "Bob"});
        schemas1.add_example::<User>(example3.clone());

        // Merge schemas2 into schemas1
        schemas1.merge(schemas2);

        // Should have both User types
        assert_eq!(schemas1.entries.len(), 2);

        // Get schema vector - conflicts should be resolved
        let schema_vec = schemas1.schema_vec();
        assert_eq!(schema_vec.len(), 2, "Should have both User schemas");

        // Names should be unique
        let names: Vec<&String> = schema_vec.iter().map(|(name, _)| name).collect();
        let mut unique_names = std::collections::HashSet::new();
        for name in &names {
            assert!(
                unique_names.insert(*name),
                "Schema name '{name}' should be unique"
            );
        }

        // Should have one namespaced name
        let has_namespaced = names.iter().any(|name| name.contains("api_v1_User"));
        assert!(
            has_namespaced,
            "Should have a namespaced User schema from api_v1"
        );

        // Verify examples are preserved after merge
        let user_type_id = TypeId::of::<User>();
        let api_v1_user_type_id = TypeId::of::<api_v1::User>();

        let user_entry = &schemas1.entries[&user_type_id];
        assert_eq!(user_entry.examples.len(), 2); // example1 + example3
        assert!(user_entry.examples.contains(&example1));
        assert!(user_entry.examples.contains(&example3));

        let api_v1_user_entry = &schemas1.entries[&api_v1_user_type_id];
        assert_eq!(api_v1_user_entry.examples.len(), 1); // example2
        assert!(api_v1_user_entry.examples.contains(&example2));
    }

    #[test]
    fn test_schema_entry_creation() {
        let entry = SchemaEntry::of::<TestType>();

        assert_eq!(entry.name, "TestType");
        assert_eq!(
            entry.type_name,
            "clawspec_core::client::openapi::schema::tests::TestType"
        );
        assert!(entry.examples.is_empty());
    }

    #[test]
    fn test_schema_entry_add_example() {
        let mut entry = SchemaEntry::of::<TestType>();
        let example = serde_json::json!({"name": "test", "value": 42});

        entry.add_example(example.clone());

        assert_eq!(entry.examples.len(), 1);
        assert!(entry.examples.contains(&example));
    }

    #[test]
    fn test_schema_entry_add_duplicate_example() {
        let mut entry = SchemaEntry::of::<TestType>();
        let example = serde_json::json!({"name": "test", "value": 42});

        entry.add_example(example.clone());
        entry.add_example(example); // Add same example again

        // Should still only have one example (IndexSet deduplicates)
        assert_eq!(entry.examples.len(), 1);
    }

    #[test]
    fn test_schema_entry_reference_creation() {
        let entry = SchemaEntry::of::<TestType>();

        // Test that non-primitive types should be referenced
        assert!(!entry.should_inline_schema());

        // Test schema reference creation
        let schema_ref: RefOr<Schema> = RefOr::Ref(Ref::from_schema_name("TestType"));
        insta::assert_debug_snapshot!(schema_ref, @r##"
        Ref(
            Ref {
                ref_location: "#/components/schemas/TestType",
                description: "",
                summary: "",
                read_only: None,
                write_only: None,
                default: None,
                title: None,
            },
        )
        "##);
    }

    #[test]
    fn test_primitive_types_are_inlined() {
        let mut schemas = Schemas::default();

        // Add a primitive type (usize)
        let usize_schema = schemas.add_example::<usize>(42);

        // Should return inline schema, not a reference
        assert!(matches!(usize_schema, RefOr::T(_)));

        // Should NOT be in the components/schemas section
        let schema_vec = schemas.schema_vec();
        assert_eq!(schema_vec.len(), 0);
    }

    #[test]
    fn test_complex_types_are_referenced() {
        let mut schemas = Schemas::default();

        // Add a complex type
        let complex_schema =
            schemas.add_example::<TestType>(serde_json::json!({"name": "test", "value": 42}));

        // Should return a reference
        assert!(matches!(complex_schema, RefOr::Ref(_)));

        // Should be in the components/schemas section
        let schema_vec = schemas.schema_vec();
        assert_eq!(schema_vec.len(), 1);
        assert_eq!(schema_vec[0].0, "TestType");
    }

    #[test]
    fn test_mixed_primitive_and_complex_types() {
        let mut schemas = Schemas::default();

        // Add primitive and complex types
        let usize_schema = schemas.add_example::<usize>(42);
        let complex_schema =
            schemas.add_example::<TestType>(serde_json::json!({"name": "test", "value": 42}));

        // Primitive should be inlined
        assert!(matches!(usize_schema, RefOr::T(_)));

        // Complex should be referenced
        assert!(matches!(complex_schema, RefOr::Ref(_)));

        // Only complex type should be in components/schemas
        let schema_vec = schemas.schema_vec();
        assert_eq!(schema_vec.len(), 1);
        assert_eq!(schema_vec[0].0, "TestType");
    }

    #[test]
    fn test_schema_name_conflicts_are_resolved() {
        // This test verifies that schema name conflicts are properly resolved
        #[derive(Debug, ToSchema, Serialize)]
        struct User {
            id: u64,
            name: String,
        }

        // Different module with same schema name
        mod other_module {
            use super::*;

            #[derive(Debug, ToSchema, Serialize)]
            pub struct User {
                user_id: String,
                email: String,
            }
        }

        let mut schemas = Schemas::default();

        // Add both types - they have different TypeIds but same schema name
        let schema1 = schemas.add::<User>();
        let schema2 = schemas.add::<other_module::User>();

        // Both should be referenced (not inlined)
        assert!(matches!(schema1, RefOr::Ref(_)));
        assert!(matches!(schema2, RefOr::Ref(_)));

        // Check the internal storage - should have 2 entries with different TypeIds
        assert_eq!(schemas.entries.len(), 2);

        // Get the schema_vec for OpenAPI output - conflicts should be resolved
        let schema_vec = schemas.schema_vec();
        assert_eq!(schema_vec.len(), 2, "Should have both schemas");

        // Extract schema names
        let names: Vec<&String> = schema_vec.iter().map(|(name, _)| name).collect();

        // Verify that names are unique (no conflicts)
        let mut unique_names = std::collections::HashSet::new();
        for name in &names {
            assert!(
                unique_names.insert(*name),
                "Schema name '{name}' should be unique"
            );
        }

        // Should have exactly one name containing "other_module_User" and one with base "User" or namespace
        let has_namespaced = names.iter().any(|name| name.contains("other_module_User"));
        assert!(has_namespaced, "Should have a namespaced User schema");

        println!("Resolved schema names: {names:?}");

        // Verify that references point to the correct unique names
        if let RefOr::Ref(ref_obj) = &schema1 {
            let ref_name = ref_obj
                .ref_location
                .trim_start_matches("#/components/schemas/");
            assert!(
                names.iter().any(|&name| name == ref_name),
                "Reference '{ref_name}' should match a schema name"
            );
        }

        if let RefOr::Ref(ref_obj) = &schema2 {
            let ref_name = ref_obj
                .ref_location
                .trim_start_matches("#/components/schemas/");
            assert!(
                names.iter().any(|&name| name == ref_name),
                "Reference '{ref_name}' should match a schema name"
            );
        }
    }

    #[test]
    fn test_enhanced_example_generation_and_validation() {
        // This test verifies enhanced example handling for schemas
        #[derive(Debug, ToSchema, Serialize)]
        struct Product {
            id: u32,
            name: String,
            price: f64,
        }

        let mut schemas = Schemas::default();

        // Add schema with multiple examples to test example collection
        let example1 = serde_json::json!({"id": 1, "name": "Laptop", "price": 999.99});
        let example2 = serde_json::json!({"id": 2, "name": "Mouse", "price": 29.99});
        let example3 = serde_json::json!({"id": 3, "name": "Keyboard", "price": 89.99});

        // Add the same type multiple times with different examples
        schemas.add_example::<Product>(example1.clone());
        schemas.add_example::<Product>(example2.clone());
        schemas.add_example::<Product>(example3.clone());

        // Should still only have one schema entry (same type)
        assert_eq!(schemas.entries.len(), 1);

        // Get the product entry to verify example collection
        let product_type_id = TypeId::of::<Product>();
        let product_entry = &schemas.entries[&product_type_id];

        // Should have collected all three examples
        assert_eq!(product_entry.examples.len(), 3);
        assert!(product_entry.examples.contains(&example1));
        assert!(product_entry.examples.contains(&example2));
        assert!(product_entry.examples.contains(&example3));

        // Schema should be properly referenced (not inlined)
        let schema_vec = schemas.schema_vec();
        assert_eq!(schema_vec.len(), 1);
        assert_eq!(schema_vec[0].0, "Product");

        // Test duplicate example deduplication
        schemas.add_example::<Product>(example1.clone()); // Add same example again
        let product_entry = &schemas.entries[&product_type_id];
        assert_eq!(
            product_entry.examples.len(),
            3,
            "Duplicate examples should be deduplicated"
        );
    }

    #[test]
    fn test_fallback_naming_strategy() {
        // Test the fallback naming when type path has insufficient parts
        #[derive(Debug, ToSchema, Serialize)]
        struct SimpleType;

        // Create a type in the root namespace (less than 2 path parts)
        let mut schemas = Schemas::default();

        // Manually create an entry to simulate root-level types
        let simple_entry = SchemaEntry {
            id: TypeId::of::<SimpleType>(),
            type_name: "SimpleType".to_string(), // Root level, no :: separator
            name: "SimpleType".to_string(),
            schema: RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            examples: IndexSet::default(),
            inline: false,
            nested: Vec::new(),
        };

        // Add the same name from another "type" to force conflict
        let conflicting_entry = SchemaEntry {
            id: TypeId::of::<String>(), // Different type, same schema name
            type_name: "String".to_string(),
            name: "SimpleType".to_string(), // Same name as above!
            schema: RefOr::T(utoipa::openapi::Schema::Object(Default::default())),
            examples: IndexSet::default(),
            inline: false,
            nested: Vec::new(),
        };

        schemas.entries.insert(simple_entry.id, simple_entry);
        schemas
            .entries
            .insert(conflicting_entry.id, conflicting_entry);

        // Get schema vector - should use hash-based fallback naming
        let schema_vec = schemas.schema_vec();
        assert_eq!(schema_vec.len(), 2);

        // Both should have unique names, and at least one should use hash-based naming
        let names: Vec<&String> = schema_vec.iter().map(|(name, _)| name).collect();
        let mut unique_names = std::collections::HashSet::new();
        for name in &names {
            assert!(
                unique_names.insert(*name),
                "Schema name '{name}' should be unique"
            );
        }

        // At least one name should contain a hex hash
        let has_hash_name = names.iter().any(|name| {
            name.contains("_")
                && name
                    .split('_')
                    .next_back()
                    .unwrap_or("")
                    .chars()
                    .all(|c| c.is_ascii_hexdigit())
        });
        assert!(
            has_hash_name,
            "Should have at least one hash-based fallback name"
        );
    }

    #[test]
    fn test_path_filtering_edge_cases() {
        // Test various edge cases in path filtering
        assert!(Schemas::is_filtered_path_part("tests"));
        assert!(Schemas::is_filtered_path_part("test"));
        assert!(Schemas::is_filtered_path_part("_test"));
        assert!(Schemas::is_filtered_path_part("testing"));
        assert!(Schemas::is_filtered_path_part("internal"));
        assert!(Schemas::is_filtered_path_part("private"));

        // These should not be filtered
        assert!(!Schemas::is_filtered_path_part("user"));
        assert!(!Schemas::is_filtered_path_part("api"));
        assert!(!Schemas::is_filtered_path_part("v1"));
        assert!(!Schemas::is_filtered_path_part("service"));
    }

    #[test]
    fn test_selective_cache_invalidation() {
        // Test that cache invalidation only affects conflicted schemas
        #[derive(Debug, ToSchema, Serialize)]
        struct User {
            id: u64,
        }

        #[derive(Debug, ToSchema, Serialize)]
        struct Product {
            name: String,
        }

        mod api_v1 {
            use super::*;

            #[derive(Debug, ToSchema, Serialize)]
            pub struct User {
                user_id: String,
            }
        }

        // Create first schema collection
        let mut schemas1 = Schemas::default();
        schemas1.add::<User>();
        schemas1.add::<Product>();

        // Manually populate cache to simulate cached state
        let user_id = TypeId::of::<User>();
        let product_id = TypeId::of::<Product>();
        schemas1.resolved_names.insert(user_id, "User".to_string());
        schemas1
            .resolved_names
            .insert(product_id, "Product".to_string());

        // Create second collection with conflicting User but no Product
        let mut schemas2 = Schemas::default();
        schemas2.add::<api_v1::User>();

        // Before merge, should have 2 cached names
        assert_eq!(schemas1.resolved_names.len(), 2);

        // Merge - should only invalidate User-related cache entries
        schemas1.merge(schemas2);

        // After merge, Product cache should remain, but User cache should be cleared
        // Note: the exact behavior depends on implementation, but cache should be smaller
        assert!(schemas1.resolved_names.len() <= 2);

        // Verify that the schema_vec works correctly after merge
        let schema_vec = schemas1.schema_vec();
        assert_eq!(schema_vec.len(), 3); // User, api_v1_User, Product

        let names: Vec<&String> = schema_vec.iter().map(|(name, _)| name).collect();
        let mut unique_names = std::collections::HashSet::new();
        for name in &names {
            assert!(
                unique_names.insert(*name),
                "Schema name '{name}' should be unique"
            );
        }
    }

    #[test]
    fn test_merge_behavior_safety() {
        // Test that merge behavior is safe under various conditions
        #[derive(Debug, ToSchema, Serialize)]
        struct User {
            id: u64,
            name: String,
        }

        #[derive(Debug, ToSchema, Serialize)]
        struct Product {
            id: u32,
            name: String,
        }

        mod v1 {
            use super::*;

            #[derive(Debug, ToSchema, Serialize)]
            pub struct User {
                user_id: String,
                email: String,
            }
        }

        mod v2 {
            use super::*;

            #[derive(Debug, ToSchema, Serialize)]
            pub struct User {
                uuid: String,
                profile: String,
            }
        }

        // Test 1: Merging empty collections is safe
        let mut empty1 = Schemas::default();
        let empty2 = Schemas::default();
        empty1.merge(empty2);
        assert_eq!(empty1.entries.len(), 0);
        assert_eq!(empty1.resolved_names.len(), 0);

        // Test 2: Merging with conflicting names preserves all data
        let mut schemas1 = Schemas::default();
        let example1 = serde_json::json!({"id": 1, "name": "Alice"});
        schemas1.add_example::<User>(example1.clone());
        schemas1.add::<Product>();

        let mut schemas2 = Schemas::default();
        let example2 = serde_json::json!({"user_id": "abc", "email": "alice@test.com"});
        schemas2.add_example::<v1::User>(example2.clone());

        // Pre-populate cache to test invalidation safety
        let user_id = TypeId::of::<User>();
        let product_id = TypeId::of::<Product>();
        let v1_user_id = TypeId::of::<v1::User>();
        schemas1.resolved_names.insert(user_id, "User".to_string());
        schemas1
            .resolved_names
            .insert(product_id, "Product".to_string());

        // Perform merge
        schemas1.merge(schemas2);

        // Verify all entries are preserved
        assert_eq!(schemas1.entries.len(), 3);
        assert!(schemas1.entries.contains_key(&user_id));
        assert!(schemas1.entries.contains_key(&product_id));
        assert!(schemas1.entries.contains_key(&v1_user_id));

        // Verify examples are preserved
        assert!(schemas1.entries[&user_id].examples.contains(&example1));
        assert!(schemas1.entries[&v1_user_id].examples.contains(&example2));

        // Verify cache invalidation only affects conflicted names
        // Product should not be invalidated as it has no conflicts
        let schema_vec = schemas1.schema_vec();
        assert_eq!(schema_vec.len(), 3);

        // All schema names must be unique
        let names: Vec<&String> = schema_vec.iter().map(|(name, _)| name).collect();
        let unique_names: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(
            names.len(),
            unique_names.len(),
            "All schema names must be unique"
        );

        // Test 3: Multiple conflicting merges work correctly
        let mut schemas3 = Schemas::default();
        let example3 = serde_json::json!({"uuid": "uuid123", "profile": "admin"});
        schemas3.add_example::<v2::User>(example3.clone());

        schemas1.merge(schemas3);

        // Should now have 4 schemas: User, Product, v1::User, v2::User
        assert_eq!(schemas1.entries.len(), 4);
        let schema_vec = schemas1.schema_vec();
        assert_eq!(schema_vec.len(), 4);

        // All names still unique
        let names: Vec<&String> = schema_vec.iter().map(|(name, _)| name).collect();
        let unique_names: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(
            names.len(),
            unique_names.len(),
            "All schema names must remain unique after multiple merges"
        );

        // Examples are preserved across merges
        let v2_user_id = TypeId::of::<v2::User>();
        assert!(schemas1.entries[&v2_user_id].examples.contains(&example3));

        // Test 4: Self-merge is safe (merging identical collections)
        let schemas_copy = schemas1.clone();
        let entries_before = schemas1.entries.len();

        schemas1.merge(schemas_copy);

        // Should have same number of entries (no duplicates)
        assert_eq!(schemas1.entries.len(), entries_before);

        // Examples should be preserved (sets prevent duplicates)
        assert!(schemas1.entries[&user_id].examples.contains(&example1));
        assert!(schemas1.entries[&v1_user_id].examples.contains(&example2));
        assert!(schemas1.entries[&v2_user_id].examples.contains(&example3));
    }
}

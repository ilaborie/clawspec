use std::any::{TypeId, type_name};
use std::collections::HashSet;
use std::fmt::Debug;
use std::sync::LazyLock;

use indexmap::{IndexMap, IndexSet};
use utoipa::ToSchema;
use utoipa::openapi::{Ref, RefOr, Schema};

/// Set of primitive type names that should be inlined rather than referenced
static PRIMITIVE_TYPES: LazyLock<HashSet<&'static str>> = LazyLock::new(|| {
    HashSet::from([
        "bool", "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128",
        "usize", "f32", "f64", "String", "str", "binary",
    ])
});

#[derive(Clone, Default)]
pub(super) struct Schemas {
    entries: IndexMap<TypeId, SchemaEntry>,
    resolved_names: std::collections::HashMap<TypeId, String>,
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
    pub(crate) fn add_entry(&mut self, entry: SchemaEntry) -> RefOr<Schema> {
        let type_id = entry.id;

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
        T: ToSchema + 'static,
    {
        let id = TypeId::of::<T>();
        self.entries.entry(id).or_insert_with(SchemaEntry::of::<T>)
    }

    pub(super) fn add<T>(&mut self) -> RefOr<Schema>
    where
        T: ToSchema + 'static,
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

    pub(super) fn add_example<T>(&mut self, example: impl Into<serde_json::Value>) -> RefOr<Schema>
    where
        T: ToSchema + 'static,
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
                .filter(|part| !part.is_empty() && *part != "tests")
                .collect();

            if type_parts.len() >= 2 {
                // Use the last two parts for namespace (e.g., "module::Type")
                format!("{}_{}", type_parts[type_parts.len() - 2], base_name)
            } else {
                // Fallback: use the TypeId as a unique suffix
                format!("{base_name}_{target_type_id:?}")
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
    pub(super) fn merge(&mut self, other: Self) {
        for (type_id, entry) in other.entries {
            self.entries
                .entry(type_id)
                .and_modify(|existing| existing.examples.extend(entry.examples.clone()))
                .or_insert(entry);
        }
        // Clear resolved names cache since we need to re-resolve after merge to handle conflicts
        self.resolved_names.clear();

        // Merge any resolved names from the other collection, but clear them since conflicts may change
        // We don't carry over resolved names because the merge might create new conflicts
    }

    pub(super) fn schema_vec(&self) -> Vec<(String, RefOr<Schema>)> {
        let mut result = vec![];

        // Create a temporary copy to resolve names
        let mut temp_schemas = self.clone();

        for (type_id, entry) in &self.entries {
            // Only include non-primitive types in the components/schemas section
            if !entry.should_inline_schema() {
                let resolved_name = temp_schemas.resolve_name_for_type(*type_id);
                let schema = entry.schema.clone();
                result.push((resolved_name, schema));
            }
        }
        result
    }

    // TODO examples - https://github.com/ilaborie/clawspec/issues/25
    // let example = Example::builder().value(output.as_example_value()?).build();
}

#[derive(Clone, derive_more::Display, derive_more::Debug)]
#[display("[{id:?}] {name}")]
pub(super) struct SchemaEntry {
    #[debug(ignore)]
    pub(super) id: TypeId,
    pub(super) type_name: String,
    pub(super) name: String,
    #[debug(ignore)]
    pub(super) schema: RefOr<Schema>,
    pub(super) examples: IndexSet<serde_json::Value>,
}

impl SchemaEntry {
    pub(crate) fn of<T>() -> Self
    where
        T: ToSchema + 'static,
    {
        let id = TypeId::of::<T>();
        let name = T::name();
        let type_name = type_name::<T>();
        Self {
            id,
            type_name: type_name.to_string(),
            name: name.to_string(),
            schema: T::schema(),
            examples: IndexSet::default(),
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
        }
    }

    pub(crate) fn add_example(&mut self, example: serde_json::Value) {
        self.examples.insert(example);
    }

    /// Determines if this schema should be inlined (for primitives) or referenced (for complex types)
    fn should_inline_schema(&self) -> bool {
        // Check if the schema name (from T::name()) is a primitive type
        // This works for both direct primitives and wrapper types like DisplayArg<T>
        // since DisplayArg<T> delegates T::name() to the inner type
        PRIMITIVE_TYPES.contains(self.name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use utoipa::ToSchema;

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
            "clawspec_core::client::schema::tests::TestType"
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
}

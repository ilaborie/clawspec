use std::any::{TypeId, type_name};
use std::fmt::Debug;

use indexmap::{IndexMap, IndexSet};
use utoipa::ToSchema;
use utoipa::openapi::{Ref, RefOr, Schema};

#[derive(Clone, Default)]
pub struct Schemas(IndexMap<TypeId, SchemaEntry>);

impl Debug for Schemas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let names = self
            .0
            .values()
            .map(|it| it.type_name.as_str())
            .collect::<Vec<_>>();
        f.debug_tuple("Schemas").field(&names).finish()
    }
}

impl Schemas {
    pub(crate) fn add_entry(&mut self, entry: SchemaEntry) -> RefOr<Schema> {
        let entry = self
            .0
            .entry(entry.id)
            .and_modify(|existing| existing.examples.extend(entry.examples.clone()))
            .or_insert(entry);
        entry.as_schema_ref()
    }

    fn add_type<T>(&mut self) -> &mut SchemaEntry
    where
        T: ToSchema + 'static,
    {
        let id = TypeId::of::<T>();
        self.0.entry(id).or_insert_with(SchemaEntry::of::<T>)
    }

    pub(super) fn add<T>(&mut self) -> RefOr<Schema>
    where
        T: ToSchema + 'static,
    {
        let entry = self.add_type::<T>();
        entry.as_schema_ref()
    }

    pub(super) fn add_example<T>(&mut self, example: impl Into<serde_json::Value>) -> RefOr<Schema>
    where
        T: ToSchema + 'static,
    {
        let example = example.into();
        let entry = self.add_type::<T>();
        entry.examples.insert(example);
        let schema_ref = entry.as_schema_ref();

        // If this is a primitive type that should be inlined, remove it from the schema collection
        // since it won't be referenced in components/schemas
        if entry.should_inline_schema() {
            let id = TypeId::of::<T>();
            self.0.shift_remove(&id);
        }

        schema_ref
    }

    pub(super) fn merge(&mut self, other: Self) {
        for (type_id, entry) in other.0 {
            self.0.insert(type_id, entry);
        }
    }

    pub(super) fn schema_vec(&self) -> Vec<(String, RefOr<Schema>)> {
        let mut result = vec![];
        for entry in self.0.values() {
            let name = entry.name.clone(); // TODO conflict - https://github.com/ilaborie/clawspec/issues/25
            let schema = entry.schema.clone();
            result.push((name, schema));
        }
        result
    }

    // TODO examples - https://github.com/ilaborie/clawspec/issues/25
    // let example = Example::builder().value(output.as_example_value()?).build();
}

#[derive(Clone, derive_more::Display, derive_more::Debug)]
#[display("[{id:?}] {name}")]
pub struct SchemaEntry {
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

    pub(crate) fn add_example(&mut self, example: serde_json::Value) {
        self.examples.insert(example);
    }

    fn as_schema_ref(&self) -> RefOr<Schema> {
        if self.should_inline_schema() {
            // Return the schema directly for primitive types
            self.schema.clone()
        } else {
            // Return a reference for complex types
            let name = &self.name; // TODO maybe conflict - https://github.com/ilaborie/clawspec/issues/25
            RefOr::Ref(Ref::from_schema_name(name))
        }
    }

    /// Determines if this schema should be inlined (for primitives) or referenced (for complex types)
    fn should_inline_schema(&self) -> bool {
        // Check for primitive types that should be inlined
        matches!(
            self.name.as_str(),
            "bool"
                | "i8"
                | "i16"
                | "i32"
                | "i64"
                | "i128"
                | "isize"
                | "u8"
                | "u16"
                | "u32"
                | "u64"
                | "u128"
                | "usize"
                | "f32"
                | "f64"
                | "String"
                | "str"
        )
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
    fn test_schema_entry_creation() {
        let entry = SchemaEntry::of::<TestType>();

        assert_eq!(entry.name, "TestType");
        assert_eq!(
            entry.type_name,
            "clawspec_utoipa::client::schema::tests::TestType"
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
    fn test_schema_entry_as_schema_ref() {
        let entry = SchemaEntry::of::<TestType>();
        let schema_ref = entry.as_schema_ref();

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
}

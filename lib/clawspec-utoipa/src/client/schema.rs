use std::any::{TypeId, type_name};

use indexmap::{IndexMap, IndexSet};
use utoipa::ToSchema;
use utoipa::openapi::{Ref, RefOr, Schema};

#[derive(Debug, Clone, Default)]
pub struct Schemas(IndexMap<TypeId, SchemaEntry>);

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
        entry.as_schema_ref()
    }

    pub(super) fn merge(&mut self, other: Self) {
        for (type_id, entry) in other.0 {
            self.0.insert(type_id, entry);
        }
    }

    pub(super) fn schema_vec(&self) -> Vec<(String, RefOr<Schema>)> {
        let mut result = vec![];
        for entry in self.0.values() {
            let name = entry.name.clone(); // TODO conflict
            let schema = entry.schema.clone();
            result.push((name, schema));
        }
        result
    }

    // TODO exeamples
    // let example = Example::builder().value(output.as_example_value()?).build();
}

#[derive(Debug, Clone, derive_more::Display)]
#[display("[{id:?}] {name}")]
pub struct SchemaEntry {
    pub(super) id: TypeId,
    pub(super) type_name: String,
    pub(super) name: String,
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
        let name = &self.name; // TODO maybe conflict
        RefOr::Ref(Ref::from_schema_name(name))
    }
}

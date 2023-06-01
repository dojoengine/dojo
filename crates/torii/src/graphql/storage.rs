use async_graphql::dynamic::{Field, FieldFuture, FieldValue, Object, TypeRef};
use async_graphql::{Name, Value};
use indexmap::IndexMap;
use sqlx::{Pool, Sqlite};

use super::{ObjectTrait, TypeMapping};

pub struct StorageObject {
    pub name: String,
    pub type_name: String,
    pub field_type_mapping: TypeMapping,
}

impl StorageObject {
    pub fn new(name: String, type_name: String, field_type_mapping: TypeMapping) -> Self {
        Self { name, type_name, field_type_mapping }
    }
}

impl ObjectTrait for StorageObject {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn field_type_mapping(&self) -> &TypeMapping {
        &self.field_type_mapping
    }

    fn field_resolvers(&self) -> Vec<Field> {
        // TODO: implement
        vec![]
    }
}

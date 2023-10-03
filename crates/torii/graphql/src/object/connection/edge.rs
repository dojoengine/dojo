use async_graphql::dynamic::TypeRef;
use async_graphql::Name;

use crate::object::ObjectTrait;
use crate::types::{GraphqlType, TypeData, TypeMapping};

pub struct EdgeObject {
    pub name: String,
    pub type_name: String,
    pub type_mapping: TypeMapping,
}

impl EdgeObject {
    pub fn new(name: String, type_name: String) -> Self {
        let type_mapping = TypeMapping::from([
            (Name::new("node"), TypeData::Simple(TypeRef::named(type_name.clone()))),
            (
                Name::new("cursor"),
                TypeData::Simple(TypeRef::named(GraphqlType::Cursor.to_string())),
            ),
        ]);

        Self {
            name: format!("{}Edge", name),
            type_name: format!("{}Edge", type_name),
            type_mapping,
        }
    }
}

impl ObjectTrait for EdgeObject {
    fn name(&self) -> &str {
        &self.name
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_mapping(&self) -> &TypeMapping {
        &self.type_mapping
    }
}

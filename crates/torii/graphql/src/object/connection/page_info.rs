use async_graphql::dynamic::TypeRef;
use async_graphql::Name;

use crate::object::{ObjectTrait, TypeMapping};
use crate::types::{GraphqlType, TypeData};

pub struct PageInfoObject {
    pub type_mapping: TypeMapping,
}

impl Default for PageInfoObject {
    fn default() -> Self {
        Self {
            type_mapping: TypeMapping::from([
                (Name::new("hasPreviousPage"), TypeData::Simple(TypeRef::named(TypeRef::BOOLEAN))),
                (Name::new("hasNextPage"), TypeData::Simple(TypeRef::named(TypeRef::BOOLEAN))),
                (
                    Name::new("startCursor"),
                    TypeData::Simple(TypeRef::named(GraphqlType::Cursor.to_string())),
                ),
                (
                    Name::new("endCursor"),
                    TypeData::Simple(TypeRef::named(GraphqlType::Cursor.to_string())),
                ),
            ]),
        }
    }
}

impl ObjectTrait for PageInfoObject {
    fn name(&self) -> &str {
        "pageInfo"
    }

    fn type_name(&self) -> &str {
        "PageInfo"
    }

    fn type_mapping(&self) -> &TypeMapping {
        &self.type_mapping
    }
}

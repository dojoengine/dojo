use async_graphql::dynamic::TypeRef;
use async_graphql::Name;
use indexmap::IndexMap;

use crate::object::{ObjectTrait, TypeMapping};
use crate::types::ScalarType;

pub struct PageInfoObject {
    pub type_mapping: TypeMapping,
}

impl Default for PageInfoObject {
    fn default() -> Self {
        Self {
            type_mapping: IndexMap::from([
                (Name::new("hasPreviousPage"), TypeRef::named(TypeRef::BOOLEAN)),
                (Name::new("hasNextPage"), TypeRef::named(TypeRef::BOOLEAN)),
                (Name::new("startCursor"), TypeRef::named(ScalarType::Cursor.to_string())),
                (Name::new("endCursor"), TypeRef::named(ScalarType::Cursor.to_string())),
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

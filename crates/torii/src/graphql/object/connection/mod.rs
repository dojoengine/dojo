use async_graphql::dynamic::{Field, InputValue, ResolverContext, TypeRef};
use async_graphql::Name;
use indexmap::IndexMap;
use sqlx::Value;

use crate::graphql::types::ScalarType;

use super::{ObjectTrait, TypeMapping, ValueMapping};

pub mod edge;
pub mod page_info;

#[derive(Debug)]
pub struct InputArguments {
    pub first: u64,
    pub last: u64,
    pub after: Option<String>,
    pub before: Option<String>,
}

#[derive(Debug)]
pub struct ConnectionData {
    pub edges: Vec<ValueMapping>,
    // pageInfo: PageInfo,
    pub total_count: u64,
}

pub struct ConnectionObject {
    pub name: String,
    pub type_name: String,
    pub type_mapping: TypeMapping,
}

impl ConnectionObject {
    pub fn new(name: String, type_name: String) -> Self {
        let type_mapping = IndexMap::from([
            (Name::new("edges"), TypeRef::named_list(format!("{}Edge", type_name))),
            (Name::new("pageInfo"), TypeRef::named_nn("PageInfo")),
            (Name::new("totalCount"), TypeRef::named_nn(TypeRef::INT)),
        ]);

        Self {
            name: format!("{}Connection", name),
            type_name: format!("{}Connection", type_name),
            type_mapping,
        }
    }

    pub fn arguments(field: Field) -> Field {
        field
            .argument(InputValue::new("first", TypeRef::named(TypeRef::INT)))
            .argument(InputValue::new("last", TypeRef::named(TypeRef::INT)))
            .argument(InputValue::new("before", TypeRef::named(ScalarType::Cursor.to_string())))
            .argument(InputValue::new("after", TypeRef::named(ScalarType::Cursor.to_string())))
    }
}

impl ObjectTrait for ConnectionObject {
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

pub fn parse_arguments(ctx: &ResolverContext<'_>) -> InputArguments {
    let first = ctx.args.try_get("first").and_then(|first| first.u64()).unwrap_or(0);
    let last = ctx.args.try_get("last").and_then(|last| last.u64()).unwrap_or(0);

    InputArguments { first, last, after: None, before: None }
}

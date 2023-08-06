use async_graphql::dynamic::{Field, InputValue, ResolverContext, TypeRef};
use async_graphql::{Error, Name, Value};
use indexmap::IndexMap;
use serde_json::Number;

use crate::graphql::types::ScalarType;

use super::{ObjectTrait, TypeMapping, ValueMapping};

pub mod edge;
pub mod page_info;

#[derive(Debug)]
pub struct InputArguments {
    pub first: Option<u64>,
    pub last: Option<u64>,
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

pub fn parse_arguments(ctx: &ResolverContext<'_>) -> Result<InputArguments, Error> {
    let first = ctx.args.try_get("first").and_then(|first| first.u64()).ok();
    let last = ctx.args.try_get("last").and_then(|last| last.u64()).ok();
    let after = ctx.args.try_get("after").and_then(|after| Ok(after.string()?.to_string())).ok();
    let before =
        ctx.args.try_get("before").and_then(|before| Ok(before.string()?.to_string())).ok();

    if first.is_some() && last.is_some() {
        return Err(
            "Passing both `first` and `last` to paginate a connection is not supported.".into()
        );
    }

    Ok(InputArguments { first, last, after, before })
}

pub fn connection_input(field: Field) -> Field {
    field
        .argument(InputValue::new("first", TypeRef::named(TypeRef::INT)))
        .argument(InputValue::new("last", TypeRef::named(TypeRef::INT)))
        .argument(InputValue::new("before", TypeRef::named(ScalarType::Cursor.to_string())))
        .argument(InputValue::new("after", TypeRef::named(ScalarType::Cursor.to_string())))
}

pub fn connection_output(
    data: &Vec<ValueMapping>,
    cursor_field: &str,
    total_count: i64,
) -> ValueMapping {
    let edges: Vec<Value> = data
        .into_iter()
        .map(|v| {
            // TODO: based64 encode cursor
            let cursor = v.get(cursor_field).expect("invalid cursor field");
            let mut edge = ValueMapping::new();
            edge.insert(Name::new("node"), Value::Object(v.clone()));
            edge.insert(Name::new("cursor"), cursor.clone());

            Value::Object(edge)
        })
        .collect();

    ValueMapping::from([
        (Name::new("totalCount"), Value::Number(Number::from(total_count))),
        (Name::new("edges"), Value::List(edges)),
    ])
}

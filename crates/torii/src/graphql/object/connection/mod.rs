use async_graphql::dynamic::{Field, InputValue, ResolverContext, TypeRef};
use async_graphql::{Error, Name, Value};
use base64::engine::general_purpose;
use base64::Engine as _;
use indexmap::IndexMap;
use serde_json::Number;

use super::{ObjectTrait, TypeMapping, ValueMapping};
use crate::graphql::types::ScalarType;
use crate::graphql::utils::extract_value::extract;

pub mod edge;
pub mod page_info;

#[derive(Debug)]
pub struct ConnectionArguments {
    pub first: Option<i64>,
    pub last: Option<i64>,
    pub after: Option<String>,
    pub before: Option<String>,
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

pub fn parse_arguments(ctx: &ResolverContext<'_>) -> Result<ConnectionArguments, Error> {
    let first = ctx.args.try_get("first").and_then(|first| first.i64()).ok();
    let last = ctx.args.try_get("last").and_then(|last| last.i64()).ok();
    let after = ctx.args.try_get("after").and_then(|after| Ok(after.string()?.to_string())).ok();
    let before =
        ctx.args.try_get("before").and_then(|before| Ok(before.string()?.to_string())).ok();

    if first.is_some() && last.is_some() {
        return Err(
            "Passing both `first` and `last` to paginate a connection is not supported.".into()
        );
    }

    if after.is_some() && before.is_some() {
        return Err(
            "Passing both `after` and `before` to paginate a connection is not supported.".into()
        );
    }

    if let Some(first) = first {
        if first < 0 {
            return Err("`first` on a connection cannot be less than zero.".into());
        }
    }

    if let Some(last) = last {
        if last < 0 {
            return Err("`last` on a connection cannot be less than zero.".into());
        }
    }

    Ok(ConnectionArguments { first, last, after, before })
}

pub fn connection_input(field: Field) -> Field {
    field
        .argument(InputValue::new("first", TypeRef::named(TypeRef::INT)))
        .argument(InputValue::new("last", TypeRef::named(TypeRef::INT)))
        .argument(InputValue::new("before", TypeRef::named(ScalarType::Cursor.to_string())))
        .argument(InputValue::new("after", TypeRef::named(ScalarType::Cursor.to_string())))
}

pub fn connection_output(data: Vec<ValueMapping>, total_count: i64) -> ValueMapping {
    let edges: Vec<Value> = data
        .into_iter()
        .map(|v| {
            let id = extract::<String>(&v, "id").expect("Invalid cursor field ID");
            let created_at =
                extract::<String>(&v, "createdAt").expect("Invalid cursor field createdAt");
            let cursor = encode_cursor(&created_at, &id);

            let mut edge = ValueMapping::new();
            edge.insert(Name::new("node"), Value::Object(v));
            edge.insert(Name::new("cursor"), Value::String(cursor));

            Value::Object(edge)
        })
        .collect();

    ValueMapping::from([
        (Name::new("totalCount"), Value::Number(Number::from(total_count))),
        (Name::new("edges"), Value::List(edges)),
    ])
}

pub fn encode_cursor(created_at: &str, id: &str) -> String {
    let cursor = format!("cursor/{}/{}", created_at, id);
    general_purpose::STANDARD.encode(cursor.as_bytes())
}

pub fn decode_cursor(cursor: String) -> Result<(String, String), Error> {
    let bytes = general_purpose::STANDARD.decode(cursor)?;
    let cursor = String::from_utf8(bytes)?;
    let parts: Vec<&str> = cursor.split('/').collect();

    if parts.len() != 3 || parts[0] != "cursor" {
        return Err("Invalid cursor format".into());
    }

    let created_at = parts[1].parse::<String>()?;
    let id = parts[2].parse::<String>()?;

    Ok((created_at, id))
}

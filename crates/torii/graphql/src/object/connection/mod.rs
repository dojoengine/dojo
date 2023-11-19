use async_graphql::connection::PageInfo;
use async_graphql::dynamic::indexmap::IndexMap;
use async_graphql::dynamic::{Field, InputValue, ResolverContext, TypeRef};
use async_graphql::{Error, Name, Value};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

use self::page_info::PageInfoObject;
use super::ObjectTrait;
use crate::constants::PAGE_INFO_TYPE_NAME;
use crate::query::order::Order;
use crate::query::value_mapping_from_row;
use crate::types::{GraphqlType, TypeData, TypeMapping, ValueMapping};
use crate::utils::extract;

pub mod cursor;
pub mod edge;
pub mod page_info;

#[derive(Debug)]
pub struct ConnectionArguments {
    pub first: Option<u64>,
    pub last: Option<u64>,
    pub after: Option<String>,
    pub before: Option<String>,
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

pub struct ConnectionObject {
    pub name: String,
    pub type_name: String,
    pub type_mapping: TypeMapping,
}

impl ConnectionObject {
    pub fn new(name: String, type_name: String) -> Self {
        let type_mapping = TypeMapping::from([
            (
                Name::new("edges"),
                TypeData::Simple(TypeRef::named_list(format!("{}Edge", type_name))),
            ),
            (Name::new("total_count"), TypeData::Simple(TypeRef::named_nn(TypeRef::INT))),
            (
                Name::new("page_info"),
                TypeData::Nested((TypeRef::named_nn(PAGE_INFO_TYPE_NAME), IndexMap::new())),
            ),
        ]);

        Self {
            name: format!("{}Connection", name),
            type_name: format!("{}Connection", type_name),
            type_mapping,
        }
    }
}

impl ObjectTrait for ConnectionObject {
    fn name(&self) -> (&str, &str) {
        (&self.name, "")
    }

    fn type_name(&self) -> &str {
        &self.type_name
    }

    fn type_mapping(&self) -> &TypeMapping {
        &self.type_mapping
    }
}

pub fn parse_connection_arguments(ctx: &ResolverContext<'_>) -> Result<ConnectionArguments, Error> {
    let first = extract::<u64>(ctx.args.as_index_map(), "first").ok();
    let last = extract::<u64>(ctx.args.as_index_map(), "last").ok();
    let after = extract::<String>(ctx.args.as_index_map(), "after").ok();
    let before = extract::<String>(ctx.args.as_index_map(), "before").ok();
    let offset = extract::<u64>(ctx.args.as_index_map(), "offset").ok();
    let limit = extract::<u64>(ctx.args.as_index_map(), "limit").ok();

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

    if (offset.is_some() || limit.is_some())
        && (first.is_some() || last.is_some() || before.is_some() || after.is_some())
    {
        return Err("Pass either `offset`/`limit` OR `first`/`last`/`after`/`before` to paginate \
                    a connection."
            .into());
    }

    Ok(ConnectionArguments { first, last, after, before, offset, limit })
}

pub fn connection_arguments(field: Field) -> Field {
    field
        .argument(InputValue::new("first", TypeRef::named(TypeRef::INT)))
        .argument(InputValue::new("last", TypeRef::named(TypeRef::INT)))
        .argument(InputValue::new("before", TypeRef::named(GraphqlType::Cursor.to_string())))
        .argument(InputValue::new("after", TypeRef::named(GraphqlType::Cursor.to_string())))
        .argument(InputValue::new("offset", TypeRef::named(TypeRef::INT)))
        .argument(InputValue::new("limit", TypeRef::named(TypeRef::INT)))
}

pub fn connection_output(
    data: &[SqliteRow],
    types: &TypeMapping,
    order: &Option<Order>,
    id_column: &str,
    total_count: i64,
    is_external: bool,
    page_info: PageInfo,
) -> sqlx::Result<ValueMapping> {
    let model_edges = data
        .iter()
        .map(|row| {
            let order_field = match order {
                Some(order) => format!("external_{}", order.field),
                None => id_column.to_string(),
            };

            let primary_order = row.try_get::<String, &str>(id_column)?;
            let secondary_order = row.try_get_unchecked::<String, &str>(&order_field)?;
            let cursor = cursor::encode(&primary_order, &secondary_order);
            let value_mapping = value_mapping_from_row(row, types, is_external)?;

            let mut edge = ValueMapping::new();
            edge.insert(Name::new("node"), Value::Object(value_mapping));
            edge.insert(Name::new("cursor"), Value::String(cursor));

            Ok(Value::Object(edge))
        })
        .collect::<sqlx::Result<Vec<Value>>>();

    Ok(ValueMapping::from([
        (Name::new("total_count"), Value::from(total_count)),
        (Name::new("edges"), Value::List(model_edges?)),
        (Name::new("page_info"), PageInfoObject::value(page_info)),
    ]))
}

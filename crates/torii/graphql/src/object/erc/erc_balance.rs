use async_graphql::connection::PageInfo;
use async_graphql::dynamic::{Field, FieldFuture, InputValue, TypeRef};
use async_graphql::{Name, Value};
use convert_case::{Case, Casing};
use serde::Deserialize;
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Pool, Row, Sqlite, SqliteConnection};
use starknet_crypto::Felt;
use torii_core::sql::utils::felt_to_sql_string;
use tracing::warn;

use super::handle_cursor;
use crate::constants::{
    DEFAULT_LIMIT, ERC_BALANCE_NAME, ERC_BALANCE_TABLE, ERC_BALANCE_TYPE_NAME, ID_COLUMN,
};
use crate::mapping::ERC_BALANCE_TYPE_MAPPING;
use crate::object::connection::page_info::PageInfoObject;
use crate::object::connection::{
    connection_arguments, cursor, parse_connection_arguments, ConnectionArguments,
};
use crate::object::{BasicObject, ResolvableObject};
use crate::query::data::count_rows;
use crate::query::filter::{Comparator, Filter, FilterValue};
use crate::query::order::{CursorDirection, Direction};
use crate::types::{TypeMapping, ValueMapping};
use crate::utils::extract;

#[derive(Debug)]
pub struct ErcBalanceObject;

impl BasicObject for ErcBalanceObject {
    fn name(&self) -> (&str, &str) {
        ERC_BALANCE_NAME
    }

    fn type_name(&self) -> &str {
        ERC_BALANCE_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &ERC_BALANCE_TYPE_MAPPING
    }
}

impl ResolvableObject for ErcBalanceObject {
    fn resolvers(&self) -> Vec<Field> {
        let account_address = "account_address";
        let argument = InputValue::new(
            account_address.to_case(Case::Camel),
            TypeRef::named_nn(TypeRef::STRING),
        );

        let mut field = Field::new(
            self.name().0,
            TypeRef::named(format!("{}Connection", self.type_name())),
            move |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let connection = parse_connection_arguments(&ctx)?;
                    let address = extract::<Felt>(
                        ctx.args.as_index_map(),
                        &account_address.to_case(Case::Camel),
                    )?;

                    let filter = vec![Filter {
                        field: "account_address".to_string(),
                        comparator: Comparator::Eq,
                        value: FilterValue::String(felt_to_sql_string(&address)),
                    }];

                    let total_count =
                        count_rows(&mut conn, ERC_BALANCE_TABLE, &None, &Some(filter)).await?;

                    let (data, page_info) =
                        fetch_erc_balances(&mut conn, address, &connection, total_count).await?;

                    let results = erc_balance_connection_output(&data, total_count, page_info)?;

                    Ok(Some(Value::Object(results)))
                })
            },
        )
        .argument(argument);

        field = connection_arguments(field);
        vec![field]
    }
}

async fn fetch_erc_balances(
    conn: &mut SqliteConnection,
    address: Felt,
    connection: &ConnectionArguments,
    total_count: i64,
) -> sqlx::Result<(Vec<SqliteRow>, PageInfo)> {
    let table_name = ERC_BALANCE_TABLE;
    let id_column = format!("b.{}", ID_COLUMN);

    let mut query = format!(
        "SELECT b.id, t.contract_address, t.name, t.symbol, t.decimals, b.balance, b.token_id, \
         c.contract_type
         FROM {table_name} b
         JOIN tokens t ON b.token_id = t.id
         JOIN contracts c ON t.contract_address = c.contract_address"
    );
    let mut conditions = vec!["b.account_address = ?".to_string()];

    let mut cursor_param = &connection.after;
    if let Some(after_cursor) = &connection.after {
        conditions.push(handle_cursor(after_cursor, CursorDirection::After, ID_COLUMN)?);
    }

    if let Some(before_cursor) = &connection.before {
        cursor_param = &connection.before;
        conditions.push(handle_cursor(before_cursor, CursorDirection::Before, ID_COLUMN)?);
    }

    if !conditions.is_empty() {
        query.push_str(&format!(" WHERE {}", conditions.join(" AND ")));
    }

    let is_cursor_based = connection.first.or(connection.last).is_some() || cursor_param.is_some();

    let data_limit =
        connection.first.or(connection.last).or(connection.limit).unwrap_or(DEFAULT_LIMIT);
    let limit = if is_cursor_based {
        match &cursor_param {
            Some(_) => data_limit + 2,
            None => data_limit + 1, // prev page does not exist
        }
    } else {
        data_limit
    };

    let order_direction = match (connection.first, connection.last) {
        (Some(_), _) => Direction::Desc,
        (_, Some(_)) => Direction::Asc,
        _ => Direction::Desc,
    };

    query.push_str(&format!(" ORDER BY {id_column} {} LIMIT {limit}", order_direction.as_ref()));

    if let Some(offset) = connection.offset {
        query.push_str(&format!(" OFFSET {}", offset));
    }

    let mut data = sqlx::query(&query).bind(felt_to_sql_string(&address)).fetch_all(conn).await?;
    let mut page_info = PageInfo {
        has_previous_page: false,
        has_next_page: false,
        start_cursor: None,
        end_cursor: None,
    };

    if data.is_empty() {
        Ok((data, page_info))
    } else if is_cursor_based {
        match cursor_param {
            Some(cursor_query) => {
                let first_cursor = cursor::encode(
                    &data[0].try_get::<String, &str>(&id_column)?,
                    &data[0].try_get_unchecked::<String, &str>(&id_column)?,
                );

                if &first_cursor == cursor_query && data.len() != 1 {
                    data.remove(0);
                    page_info.has_previous_page = true;
                } else {
                    data.pop();
                }

                if data.len() as u64 == limit - 1 {
                    page_info.has_next_page = true;
                    data.pop();
                }
            }
            None => {
                if data.len() as u64 == limit {
                    page_info.has_next_page = true;
                    data.pop();
                }
            }
        }

        if !data.is_empty() {
            page_info.start_cursor = Some(cursor::encode(
                &data[0].try_get::<String, &str>(ID_COLUMN)?,
                &data[0].try_get_unchecked::<String, &str>(ID_COLUMN)?,
            ));
            page_info.end_cursor = Some(cursor::encode(
                &data[data.len() - 1].try_get::<String, &str>(ID_COLUMN)?,
                &data[data.len() - 1].try_get_unchecked::<String, &str>(ID_COLUMN)?,
            ));
        }

        Ok((data, page_info))
    } else {
        let offset = connection.offset.unwrap_or(0);
        if 1 < offset && offset < total_count as u64 {
            page_info.has_previous_page = true;
        }
        if limit + offset < total_count as u64 {
            page_info.has_next_page = true;
        }

        Ok((data, page_info))
    }
}

fn erc_balance_connection_output(
    data: &[SqliteRow],
    total_count: i64,
    page_info: PageInfo,
) -> sqlx::Result<ValueMapping> {
    let mut edges = Vec::new();
    for row in data {
        let row = BalanceQueryResultRaw::from_row(row)?;
        let cursor = cursor::encode(&row.id, &row.id);

        let balance_value = match row.contract_type.to_lowercase().as_str() {
            "erc20" => {
                let token_metadata = Value::Object(ValueMapping::from([
                    (Name::new("name"), Value::String(row.name)),
                    (Name::new("symbol"), Value::String(row.symbol)),
                    // for erc20 there is no token_id
                    (Name::new("tokenId"), Value::Null),
                    (Name::new("decimals"), Value::String(row.decimals.to_string())),
                    (Name::new("contractAddress"), Value::String(row.contract_address.clone())),
                ]));

                Value::Object(ValueMapping::from([
                    (Name::new("balance"), Value::String(row.balance)),
                    (Name::new("type"), Value::String(row.contract_type)),
                    (Name::new("tokenMetadata"), token_metadata),
                ]))
            }
            "erc721" => {
                // contract_address:token_id
                let token_id = row.token_id.split(':').collect::<Vec<&str>>();
                assert!(token_id.len() == 2);

                let token_metadata = Value::Object(ValueMapping::from([
                    (Name::new("contractAddress"), Value::String(row.contract_address.clone())),
                    (Name::new("name"), Value::String(row.name)),
                    (Name::new("symbol"), Value::String(row.symbol)),
                    (Name::new("tokenId"), Value::String(token_id[1].to_string())),
                    (Name::new("decimals"), Value::String(row.decimals.to_string())),
                ]));

                Value::Object(ValueMapping::from([
                    (Name::new("balance"), Value::String(row.balance)),
                    (Name::new("type"), Value::String(row.contract_type)),
                    (Name::new("tokenMetadata"), token_metadata),
                ]))
            }
            _ => {
                warn!("Unknown contract type: {}", row.contract_type);
                continue;
            }
        };

        edges.push(Value::Object(ValueMapping::from([
            (Name::new("node"), balance_value),
            (Name::new("cursor"), Value::String(cursor)),
        ])));
    }

    Ok(ValueMapping::from([
        (Name::new("totalCount"), Value::from(total_count)),
        (Name::new("edges"), Value::List(edges)),
        (Name::new("pageInfo"), PageInfoObject::value(page_info)),
    ]))
}

// TODO: This would be required when subscriptions are needed
// impl ErcBalanceObject {
//     pub fn value_mapping(entity: ErcBalance) -> ValueMapping {
//         IndexMap::from([
//         ])
//     }
// }

#[derive(FromRow, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct BalanceQueryResultRaw {
    pub id: String,
    pub contract_address: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub token_id: String,
    pub balance: String,
    pub contract_type: String,
}

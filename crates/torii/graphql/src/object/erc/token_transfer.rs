use async_graphql::connection::PageInfo;
use async_graphql::dynamic::{Field, FieldFuture, InputValue, TypeRef};
use async_graphql::{Name, Value};
use convert_case::{Case, Casing};
use serde::Deserialize;
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Pool, Row, Sqlite, SqliteConnection};
use starknet_crypto::Felt;
use torii_core::constants::TOKEN_TRANSFER_TABLE;
use torii_core::engine::get_transaction_hash_from_event_id;
use torii_core::sql::utils::felt_to_sql_string;
use tracing::warn;

use super::handle_cursor;
use crate::constants::{DEFAULT_LIMIT, ID_COLUMN, TOKEN_TRANSFER_NAME, TOKEN_TRANSFER_TYPE_NAME};
use crate::mapping::TOKEN_TRANSFER_TYPE_MAPPING;
use crate::object::connection::page_info::PageInfoObject;
use crate::object::connection::{
    connection_arguments, cursor, parse_connection_arguments, ConnectionArguments,
};
use crate::object::{BasicObject, ResolvableObject};
use crate::query::order::{CursorDirection, Direction};
use crate::types::{TypeMapping, ValueMapping};
use crate::utils::extract;

#[derive(Debug)]
pub struct ErcTransferObject;

impl BasicObject for ErcTransferObject {
    fn name(&self) -> (&str, &str) {
        TOKEN_TRANSFER_NAME
    }

    fn type_name(&self) -> &str {
        TOKEN_TRANSFER_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &TOKEN_TRANSFER_TYPE_MAPPING
    }
}

impl ResolvableObject for ErcTransferObject {
    fn resolvers(&self) -> Vec<Field> {
        let account_address = "account_address";
        let arg_addr = InputValue::new(
            account_address.to_case(Case::Camel),
            TypeRef::named_nn(TypeRef::STRING),
        );

        let mut field = Field::new(
            self.name().1,
            TypeRef::named(format!("{}Connection", self.type_name())),
            move |ctx| {
                FieldFuture::new(async move {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                    let connection = parse_connection_arguments(&ctx)?;
                    let address = extract::<Felt>(
                        ctx.args.as_index_map(),
                        &account_address.to_case(Case::Camel),
                    )?;

                    let total_count: (i64,) = sqlx::query_as(&format!(
                        "SELECT COUNT(*) FROM {TOKEN_TRANSFER_TABLE} WHERE from_address = ? OR \
                         to_address = ?"
                    ))
                    .bind(felt_to_sql_string(&address))
                    .bind(felt_to_sql_string(&address))
                    .fetch_one(&mut *conn)
                    .await?;
                    let total_count = total_count.0;

                    let (data, page_info) =
                        fetch_token_transfers(&mut conn, address, &connection, total_count).await?;
                    let results = token_transfers_connection_output(&data, total_count, page_info)?;

                    Ok(Some(Value::Object(results)))
                })
            },
        )
        .argument(arg_addr);

        field = connection_arguments(field);
        vec![field]
    }
}

async fn fetch_token_transfers(
    conn: &mut SqliteConnection,
    address: Felt,
    connection: &ConnectionArguments,
    total_count: i64,
) -> sqlx::Result<(Vec<SqliteRow>, PageInfo)> {
    let table_name = TOKEN_TRANSFER_TABLE;
    let id_column = format!("et.{}", ID_COLUMN);

    let mut query = format!(
        r#"
SELECT
    et.id,
    et.contract_address,
    et.from_address,
    et.to_address,
    et.amount,
    et.token_id,
    et.executed_at,
    t.name,
    t.symbol,
    t.decimals,
    c.contract_type,
    t.metadata
FROM
    {table_name} et
JOIN
    tokens t ON et.token_id = t.id
JOIN
    contracts c ON t.contract_address = c.contract_address
"#,
    );

    let mut conditions = vec!["et.from_address = ? OR et.to_address = ?".to_string()];

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

    let mut data = sqlx::query(&query)
        .bind(felt_to_sql_string(&address))
        .bind(felt_to_sql_string(&address))
        .fetch_all(conn)
        .await?;

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
                    &data[0].try_get::<String, &str>(ID_COLUMN)?,
                    &data[0].try_get_unchecked::<String, &str>(ID_COLUMN)?,
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

fn token_transfers_connection_output(
    data: &[SqliteRow],
    total_count: i64,
    page_info: PageInfo,
) -> sqlx::Result<ValueMapping> {
    let mut edges = Vec::new();

    for row in data {
        let row = TransferQueryResultRaw::from_row(row)?;
        let transaction_hash = get_transaction_hash_from_event_id(&row.id);
        let cursor = cursor::encode(&row.id, &row.id);

        let transfer_value = match row.contract_type.to_lowercase().as_str() {
            "erc20" => {
                let token_metadata = Value::Object(ValueMapping::from([
                    (Name::new("name"), Value::String(row.name)),
                    (Name::new("symbol"), Value::String(row.symbol)),
                    // for erc20 there is no token_id
                    (Name::new("tokenId"), Value::Null),
                    (Name::new("decimals"), Value::String(row.decimals.to_string())),
                    (Name::new("contractAddress"), Value::String(row.contract_address.clone())),
                    (Name::new("erc721"), Value::Null),
                ]));

                Value::Object(ValueMapping::from([
                    (Name::new("from"), Value::String(row.from_address)),
                    (Name::new("to"), Value::String(row.to_address)),
                    (Name::new("amount"), Value::String(row.amount)),
                    (Name::new("type"), Value::String(row.contract_type)),
                    (Name::new("executedAt"), Value::String(row.executed_at)),
                    (Name::new("tokenMetadata"), token_metadata),
                    (Name::new("transactionHash"), Value::String(transaction_hash)),
                ]))
            }
            "erc721" => {
                // contract_address:token_id
                let token_id = row.token_id.split(':').collect::<Vec<&str>>();
                assert!(token_id.len() == 2);

                let image_path = format!("{}/{}", token_id.join("/"), "image");
                let metadata: serde_json::Value =
                    serde_json::from_str(&row.metadata).expect("metadata is always json");
                let erc721_name =
                    metadata.get("name").map(|v| v.to_string().trim_matches('"').to_string());
                let erc721_description = metadata
                    .get("description")
                    .map(|v| v.to_string().trim_matches('"').to_string());
                let erc721_attributes =
                    metadata.get("attributes").map(|v| v.to_string().trim_matches('"').to_string());

                let token_metadata = Value::Object(ValueMapping::from([
                    (Name::new("name"), Value::String(row.name)),
                    (Name::new("symbol"), Value::String(row.symbol)),
                    (Name::new("decimals"), Value::String(row.decimals.to_string())),
                    (Name::new("contractAddress"), Value::String(row.contract_address.clone())),
                    (
                        Name::new("erc721"),
                        Value::Object(ValueMapping::from([
                            (Name::new("imagePath"), Value::String(image_path)),
                            (Name::new("tokenId"), Value::String(token_id[1].to_string())),
                            (Name::new("metadata"), Value::String(row.metadata)),
                            (
                                Name::new("name"),
                                erc721_name.map(Value::String).unwrap_or(Value::Null),
                            ),
                            (
                                Name::new("description"),
                                erc721_description.map(Value::String).unwrap_or(Value::Null),
                            ),
                            (
                                Name::new("attributes"),
                                erc721_attributes.map(Value::String).unwrap_or(Value::Null),
                            ),
                        ])),
                    ),
                ]));

                Value::Object(ValueMapping::from([
                    (Name::new("from"), Value::String(row.from_address)),
                    (Name::new("to"), Value::String(row.to_address)),
                    (Name::new("amount"), Value::String(row.amount)),
                    (Name::new("type"), Value::String(row.contract_type)),
                    (Name::new("executedAt"), Value::String(row.executed_at)),
                    (Name::new("tokenMetadata"), token_metadata),
                    (Name::new("transactionHash"), Value::String(transaction_hash)),
                ]))
            }
            _ => {
                warn!("Unknown contract type: {}", row.contract_type);
                continue;
            }
        };

        edges.push(Value::Object(ValueMapping::from([
            (Name::new("node"), transfer_value),
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
// impl ErcTransferObject {
//     pub fn value_mapping(entity: ErcBalance) -> ValueMapping {
//         IndexMap::from([
//         ])
//     }
// }

#[derive(FromRow, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct TransferQueryResultRaw {
    pub id: String,
    pub contract_address: String,
    pub from_address: String,
    pub to_address: String,
    pub token_id: String,
    pub amount: String,
    pub executed_at: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub contract_type: String,
    pub metadata: String,
}

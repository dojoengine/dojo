use async_graphql::connection::PageInfo;
use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputValue, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use convert_case::{Case, Casing};
use serde::Deserialize;
use sqlx::sqlite::SqliteRow;
use sqlx::{FromRow, Pool, Row, Sqlite, SqliteConnection};
use starknet_crypto::Felt;
use tokio_stream::StreamExt;
use torii_sqlite::constants::TOKEN_BALANCE_TABLE;
use torii_sqlite::simple_broker::SimpleBroker;
use torii_sqlite::types::TokenBalance;
use torii_sqlite::utils::felt_to_sql_string;
use tracing::warn;

use super::erc_token::{Erc20Token, ErcTokenType};
use super::{handle_cursor, Connection, ConnectionEdge};
use crate::constants::{DEFAULT_LIMIT, ID_COLUMN, TOKEN_BALANCE_NAME, TOKEN_BALANCE_TYPE_NAME};
use crate::mapping::TOKEN_BALANCE_TYPE_MAPPING;
use crate::object::connection::page_info::PageInfoObject;
use crate::object::connection::{
    connection_arguments, cursor, parse_connection_arguments, ConnectionArguments,
};
use crate::object::erc::erc_token::{Erc1155Token, Erc721Token};
use crate::object::{BasicObject, ResolvableObject};
use crate::query::data::count_rows;
use crate::query::filter::{Comparator, Filter, FilterValue};
use crate::query::order::{CursorDirection, Direction};
use crate::types::TypeMapping;
use crate::utils::extract;

#[derive(Debug)]
pub struct ErcBalanceObject;

impl BasicObject for ErcBalanceObject {
    fn name(&self) -> (&str, &str) {
        TOKEN_BALANCE_NAME
    }

    fn type_name(&self) -> &str {
        TOKEN_BALANCE_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &TOKEN_BALANCE_TYPE_MAPPING
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

                    let filter = vec![Filter {
                        field: "account_address".to_string(),
                        comparator: Comparator::Eq,
                        value: FilterValue::String(felt_to_sql_string(&address)),
                    }];

                    let total_count =
                        count_rows(&mut conn, TOKEN_BALANCE_TABLE, &None, &Some(filter)).await?;

                    let (data, page_info) =
                        fetch_token_balances(&mut conn, address, &connection, total_count).await?;

                    let results = token_balances_connection_output(&data, total_count, page_info)?;

                    Ok(Some(results))
                })
            },
        )
        .argument(argument);

        field = connection_arguments(field);
        vec![field]
    }

    fn subscriptions(&self) -> Option<Vec<SubscriptionField>> {
        Some(vec![
            SubscriptionField::new(
                "tokenBalanceUpdated",
                TypeRef::named_nn(self.type_name()),
                |ctx| {
                    SubscriptionFieldFuture::new(async move {
                        let address = match ctx.args.get("accountAddress") {
                            Some(addr) => Some(addr.string()?.to_string()),
                            None => None,
                        };

                        let pool = ctx.data::<Pool<Sqlite>>()?;
                        Ok(SimpleBroker::<TokenBalance>::subscribe()
                            .then(move |token_balance| {
                                let address = address.clone();
                                let pool = pool.clone();
                                async move {
                                    // Filter by account address if provided
                                    if let Some(addr) = &address {
                                        if token_balance.account_address != *addr {
                                            return None;
                                        }
                                    }

                                    // Fetch associated token data
                                    let query = format!(
                                        "SELECT b.id, t.contract_address, t.name, t.symbol, \
                                         t.decimals, b.balance, b.token_id, t.metadata, \
                                         c.contract_type
                                        FROM {} b
                                        JOIN tokens t ON b.token_id = t.id
                                        JOIN contracts c ON t.contract_address = \
                                         c.contract_address
                                        WHERE b.id = ?",
                                        TOKEN_BALANCE_TABLE
                                    );

                                    let row = match sqlx::query(&query)
                                        .bind(&token_balance.id)
                                        .fetch_one(&pool)
                                        .await
                                    {
                                        Ok(row) => row,
                                        Err(_) => return None,
                                    };

                                    let row = match BalanceQueryResultRaw::from_row(&row) {
                                        Ok(row) => row,
                                        Err(_) => return None,
                                    };

                                    // Use the extracted mapping function
                                    match token_balance_mapping_from_row(&row) {
                                        Ok(balance_value) => Some(Ok(FieldValue::owned_any(balance_value))),
                                        Err(err) => {
                                            warn!("Failed to transform row to token balance in subscription: {}", err);
                                            None
                                        }
                                    }
                                }
                            })
                            .filter_map(|result| result))
                    })
                },
            )
            .argument(InputValue::new("accountAddress", TypeRef::named(TypeRef::STRING))),
        ])
    }
}

async fn fetch_token_balances(
    conn: &mut SqliteConnection,
    address: Felt,
    connection: &ConnectionArguments,
    total_count: i64,
) -> sqlx::Result<(Vec<SqliteRow>, PageInfo)> {
    let table_name = TOKEN_BALANCE_TABLE;
    let id_column = format!("b.{}", ID_COLUMN);

    let mut query = format!(
        "SELECT b.id, t.contract_address, t.name, t.symbol, t.decimals, b.balance, b.token_id, \
         t.metadata, c.contract_type
         FROM {table_name} b
         JOIN tokens t ON b.token_id = t.id
         JOIN contracts c ON t.contract_address = c.contract_address"
    );

    // Only select balances for the given account address and non-zero balances.
    let mut conditions = vec![
        "(b.account_address = ?)".to_string(),
        "b.balance != '0x0000000000000000000000000000000000000000000000000000000000000000'"
            .to_string(),
    ];

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

fn token_balances_connection_output<'a>(
    data: &[SqliteRow],
    total_count: i64,
    page_info: PageInfo,
) -> sqlx::Result<FieldValue<'a>> {
    let mut edges = Vec::new();
    for row in data {
        let row = BalanceQueryResultRaw::from_row(row)?;
        let cursor = cursor::encode(&row.id, &row.id);

        match token_balance_mapping_from_row(&row) {
            Ok(balance_value) => {
                edges.push(ConnectionEdge { node: balance_value, cursor });
            },
            Err(err) => {
                warn!("Failed to transform row to token balance: {}", err);
                continue;
            }
        }
    }

    Ok(FieldValue::owned_any(Connection {
        total_count,
        edges,
        page_info: PageInfoObject::value(page_info),
    }))
}

/// Transforms a BalanceQueryResultRaw into an ErcTokenType
fn token_balance_mapping_from_row(row: &BalanceQueryResultRaw) -> Result<ErcTokenType, String> {
    match row.contract_type.to_lowercase().as_str() {
        "erc20" => {
            let token_metadata = Erc20Token {
                contract_address: row.contract_address.clone(),
                name: row.name.clone(),
                symbol: row.symbol.clone(),
                decimals: row.decimals,
                amount: row.balance.clone(),
            };

            Ok(ErcTokenType::Erc20(token_metadata))
        }
        "erc721" => {
            // contract_address:token_id
            let token_id = row.token_id.split(':').collect::<Vec<&str>>();
            if token_id.len() != 2 {
                return Err(format!("Invalid token_id format: {}", row.token_id));
            }

            let metadata_str = &row.metadata;
            let (
                metadata_str,
                metadata_name,
                metadata_description,
                metadata_attributes,
                image_path,
            ) = if metadata_str.is_empty() {
                (String::new(), None, None, None, String::new())
            } else {
                let metadata: serde_json::Value = match serde_json::from_str(metadata_str) {
                    Ok(value) => value,
                    Err(e) => return Err(format!("Failed to parse metadata as JSON: {}", e)),
                };
                
                let metadata_name =
                    metadata.get("name").map(|v| v.to_string().trim_matches('"').to_string());
                let metadata_description = metadata
                    .get("description")
                    .map(|v| v.to_string().trim_matches('"').to_string());
                let metadata_attributes = metadata
                    .get("attributes")
                    .map(|v| v.to_string().trim_matches('"').to_string());

                let image_path = format!("{}/{}", token_id.join("/"), "image");

                (
                    metadata_str.to_owned(),
                    metadata_name,
                    metadata_description,
                    metadata_attributes,
                    image_path,
                )
            };

            let token_metadata = Erc721Token {
                name: row.name.clone(),
                metadata: metadata_str,
                contract_address: row.contract_address.clone(),
                symbol: row.symbol.clone(),
                token_id: token_id[1].to_string(),
                metadata_name,
                metadata_description,
                metadata_attributes,
                image_path,
            };

            Ok(ErcTokenType::Erc721(token_metadata))
        }
        "erc1155" => {
            // contract_address:token_id
            let token_id = row.token_id.split(':').collect::<Vec<&str>>();
            if token_id.len() != 2 {
                return Err(format!("Invalid token_id format: {}", row.token_id));
            }

            let metadata_str = &row.metadata;
            let (
                metadata_str,
                metadata_name,
                metadata_description,
                metadata_attributes,
                image_path,
            ) = if metadata_str.is_empty() {
                (String::new(), None, None, None, String::new())
            } else {
                let metadata: serde_json::Value = match serde_json::from_str(metadata_str) {
                    Ok(value) => value,
                    Err(e) => return Err(format!("Failed to parse metadata as JSON: {}", e)),
                };
                
                let metadata_name =
                    metadata.get("name").map(|v| v.to_string().trim_matches('"').to_string());
                let metadata_description = metadata
                    .get("description")
                    .map(|v| v.to_string().trim_matches('"').to_string());
                let metadata_attributes = metadata
                    .get("attributes")
                    .map(|v| v.to_string().trim_matches('"').to_string());

                let image_path = format!("{}/{}", token_id.join("/"), "image");

                (
                    metadata_str.to_owned(),
                    metadata_name,
                    metadata_description,
                    metadata_attributes,
                    image_path,
                )
            };

            let token_metadata = Erc1155Token {
                name: row.name.clone(),
                metadata: metadata_str,
                contract_address: row.contract_address.clone(),
                symbol: row.symbol.clone(),
                token_id: token_id[1].to_string(),
                amount: row.balance.clone(),
                metadata_name,
                metadata_description,
                metadata_attributes,
                image_path,
            };

            Ok(ErcTokenType::Erc1155(token_metadata))
        }
        _ => Err(format!("Unknown contract type: {}", row.contract_type)),
    }
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
    pub metadata: String,
}

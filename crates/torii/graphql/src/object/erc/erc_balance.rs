use async_graphql::dynamic::{Field, FieldFuture, InputValue, TypeRef};
use async_graphql::{Name, Value};
use convert_case::{Case, Casing};
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite, SqliteConnection};
use starknet_crypto::Felt;
use torii_core::sql::utils::felt_to_sql_string;
use tracing::warn;

use crate::constants::{ERC_BALANCE_NAME, ERC_BALANCE_TYPE_NAME};
use crate::mapping::ERC_BALANCE_TYPE_MAPPING;
use crate::object::{BasicObject, ResolvableObject};
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

        let field = Field::new(self.name().0, TypeRef::named_list(self.type_name()), move |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let address = extract::<Felt>(
                    ctx.args.as_index_map(),
                    &account_address.to_case(Case::Camel),
                )?;

                let erc_balances = fetch_erc_balances(&mut conn, address).await?;

                Ok(Some(Value::List(erc_balances)))
            })
        })
        .argument(argument);
        vec![field]
    }
}

async fn fetch_erc_balances(
    conn: &mut SqliteConnection,
    address: Felt,
) -> sqlx::Result<Vec<Value>> {
    let query = "SELECT t.contract_address, t.name, t.symbol, t.decimals, b.balance, b.token_id, \
                 c.contract_type
         FROM balances b
         JOIN tokens t ON b.token_id = t.id
         JOIN contracts c ON t.contract_address = c.contract_address
         WHERE b.account_address = ?";

    let rows = sqlx::query(query).bind(felt_to_sql_string(&address)).fetch_all(conn).await?;

    let mut erc_balances = Vec::new();

    for row in rows {
        let row = BalanceQueryResultRaw::from_row(&row)?;

        let balance_value = match row.contract_type.to_lowercase().as_str() {
            "erc20" => {
                let token_metadata = Value::Object(ValueMapping::from([
                    (Name::new("name"), Value::String(row.name)),
                    (Name::new("symbol"), Value::String(row.symbol)),
                    // for erc20 there is no token_id
                    (Name::new("token_id"), Value::Null),
                    (Name::new("decimals"), Value::String(row.decimals.to_string())),
                    (Name::new("contract_address"), Value::String(row.contract_address.clone())),
                ]));

                Value::Object(ValueMapping::from([
                    (Name::new("balance"), Value::String(row.balance)),
                    (Name::new("type"), Value::String(row.contract_type)),
                    (Name::new("token_metadata"), token_metadata),
                ]))
            }
            "erc721" => {
                // contract_address:token_id
                let token_id = row.token_id.split(':').collect::<Vec<&str>>();
                assert!(token_id.len() == 2);

                let token_metadata = Value::Object(ValueMapping::from([
                    (Name::new("contract_address"), Value::String(row.contract_address.clone())),
                    (Name::new("name"), Value::String(row.name)),
                    (Name::new("symbol"), Value::String(row.symbol)),
                    (Name::new("token_id"), Value::String(token_id[1].to_string())),
                    (Name::new("decimals"), Value::String(row.decimals.to_string())),
                ]));

                Value::Object(ValueMapping::from([
                    (Name::new("balance"), Value::String(row.balance)),
                    (Name::new("type"), Value::String(row.contract_type)),
                    (Name::new("token_metadata"), token_metadata),
                ]))
            }
            _ => {
                warn!("Unknown contract type: {}", row.contract_type);
                continue;
            }
        };

        erc_balances.push(balance_value);
    }

    Ok(erc_balances)
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
    pub contract_address: String,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub token_id: String,
    pub balance: String,
    pub contract_type: String,
}

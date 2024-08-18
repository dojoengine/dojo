use crate::types::ValueMapping;
use crate::utils::extract;
use async_graphql::dynamic::{Field, FieldFuture, InputValue, TypeRef};
use async_graphql::{Name, Value};
use convert_case::{Case, Casing};
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite, SqliteConnection};
use tracing::warn;

use crate::constants::{ERC_BALANCE_NAME, ERC_BALANCE_TYPE_NAME};
use crate::mapping::ERC_BALANCE_TYPE_MAPPING;
use crate::object::{BasicObject, ResolvableObject};
use crate::types::TypeMapping;

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
                let address: String = extract::<String>(
                    ctx.args.as_index_map(),
                    &account_address.to_case(Case::Camel),
                )?;

                let erc_balances = fetch_erc_balances(&mut conn, &address).await?;

                Ok(Some(Value::List(erc_balances)))
            })
        })
        .argument(argument);
        vec![field]
    }
}

async fn fetch_erc_balances(
    conn: &mut SqliteConnection,
    address: &str,
) -> sqlx::Result<Vec<Value>> {
    let query = "SELECT t.contract_address, t.name, t.symbol, t.decimals, b.balance, b.token_id, c.contract_type 
         FROM balances b
         JOIN tokens t ON b.token_id = t.id
         JOIN contracts c ON t.contract_address = c.contract_address
         WHERE b.account_address = ?";

    let rows = sqlx::query(query).bind(address).fetch_all(conn).await?;

    let mut erc_balances = Vec::new();

    for row in rows {
        let row = BalanceQueryResultRaw::from_row(&row)?;

        let balance_value = match row.contract_type.as_str() {
            "ERC20" | "Erc20" | "erc20" | "ERC721" | "Erc721" | "erc721" => {
                let token_metadata = Value::Object(ValueMapping::from([
                    (Name::new("contract_address"), Value::String(row.contract_address.clone())),
                    (Name::new("name"), Value::String(row.name)),
                    (Name::new("symbol"), Value::String(row.symbol)),
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
    pub balance: String,
    pub contract_type: String,
}

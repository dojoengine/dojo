use crate::types::ValueMapping;
use crate::utils::extract;
use async_graphql::dynamic::{Field, FieldFuture, TypeRef};
use async_graphql::{Name, Value};
use convert_case::{Case, Casing};
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite, SqliteConnection};

use super::{BasicObject, ResolvableObject, TypeMapping};
use crate::constants::{ERC20_BALANCE_TABLE, ERC_BALANCE_NAMES, ERC_BALANCE_TYPE_NAME};
use crate::mapping::ERC_BALANCE_TYPE_MAPPING;

#[derive(Debug)]
pub struct ErcBalanceObject;

impl BasicObject for ErcBalanceObject {
    fn name(&self) -> (&str, &str) {
        ERC_BALANCE_NAMES
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
        let id_column = "";
        let field = Field::new(self.name().0, TypeRef::named(self.type_name()), move |ctx| {
            FieldFuture::new(async move {
                // read address to be queried
                // query data from tables
                // return as graphql object
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let address: String =
                    extract::<String>(ctx.args.as_index_map(), &id_column.to_case(Case::Camel))?;

                let erc20_balances = fetch_erc20_balances(&mut conn, &address).await?;
                // let erc721_balances = fetch_erc721_balances(&mut conn, &address).await?;

                let result = ValueMapping::from([
                    (Name::new("address"), Value::String(address)),
                    (Name::new("erc20_balances"), Value::List(erc20_balances)),
                    (Name::new("erc721_balances"), Value::List(vec![])),
                ]);

                Ok(Some(Value::Object(result)))
            })
        });
        vec![field]
    }
}

async fn fetch_erc721_balances(
    conn: &mut SqliteConnection,
    address: &str,
) -> sqlx::Result<Vec<Value>> {
    todo!()
}

// Collects data from erc20_balances table
// It doesn't contain contract metadata like name, symbol, decimals yet, they will be
// added to the value object later on
async fn fetch_erc20_balances(
    conn: &mut SqliteConnection,
    address: &str,
) -> sqlx::Result<Vec<Value>> {
    let query = format!("SELECT * FROM {} WHERE address = ?", ERC20_BALANCE_TABLE);
    let res = sqlx::query(&query).bind(address).fetch_all(conn).await?;
    res.into_iter()
        .map(|row| {
            let erc20_balance = Erc20Balance::from_row(&row)?;
            Ok(Value::Object(ValueMapping::from([
                (Name::new("address"), Value::String(erc20_balance.address)),
                (Name::new("tokenAddress"), Value::String(erc20_balance.token_address)),
                (Name::new("balance"), Value::String(erc20_balance.balance)),
            ])))
        })
        .collect()
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
pub struct Erc20Balance {
    pub name: String,
    pub symbol: String,
    pub address: String,
    pub decimals: u8,
    pub token_address: String,
    pub balance: String,
}

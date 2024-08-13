use crate::types::ValueMapping;
use crate::utils::extract;
use async_graphql::dynamic::{Field, FieldFuture, InputValue, TypeRef};
use async_graphql::{Name, Value};
use convert_case::{Case, Casing};
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite, SqliteConnection};

use super::{BasicObject, ResolvableObject, TypeMapping};
use crate::constants::{
    ERC20_BALANCE_TABLE, ERC721_BALANCE_TABLE, ERC_BALANCE_NAMES, ERC_BALANCE_TYPE_NAME,
};
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
        let account_address = "account_address";
        let argument = InputValue::new(
            account_address.to_case(Case::Camel),
            TypeRef::named_nn(TypeRef::STRING),
        );

        let field = Field::new(self.name().0, TypeRef::named(self.type_name()), move |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let address: String = extract::<String>(
                    ctx.args.as_index_map(),
                    &account_address.to_case(Case::Camel),
                )?;

                let erc20_balances = fetch_erc20_balances(&mut conn, &address).await?;
                let erc721_balances = fetch_erc721_balances(&mut conn, &address).await?;

                let result = ValueMapping::from([
                    (Name::new("accountAddress"), Value::String(address)),
                    (Name::new("erc20"), Value::List(erc20_balances)),
                    (Name::new("erc721"), Value::List(erc721_balances)),
                ]);

                Ok(Some(Value::Object(result)))
            })
        })
        .argument(argument);
        vec![field]
    }
}

async fn fetch_erc721_balances(
    conn: &mut SqliteConnection,
    address: &str,
) -> sqlx::Result<Vec<Value>> {
    let query = format!("SELECT * FROM {} WHERE account_address = ?", ERC721_BALANCE_TABLE);
    let res = sqlx::query(&query).bind(address).fetch_all(conn).await?;
    res.into_iter()
        .map(|row| {
            let erc721_balance = Erc721BalanceRaw::from_row(&row)?;
            Ok(Value::Object(ValueMapping::from([
                (Name::new("accountAddress"), Value::String(erc721_balance.account_address)),
                (Name::new("tokenAddress"), Value::String(erc721_balance.token_address)),
                (Name::new("tokenId"), Value::String(erc721_balance.token_id)),
            ])))
        })
        .collect()
}

// Collects data from erc20_balances table
// It doesn't contain contract metadata like name, symbol, decimals yet, they will be
// added to the value object later on
async fn fetch_erc20_balances(
    conn: &mut SqliteConnection,
    address: &str,
) -> sqlx::Result<Vec<Value>> {
    let query = format!("SELECT * FROM {} WHERE account_address = ?", ERC20_BALANCE_TABLE);
    let res = sqlx::query(&query).bind(address).fetch_all(conn).await?;
    res.into_iter()
        .map(|row| {
            let erc20_balance = Erc20BalanceRaw::from_row(&row)?;
            Ok(Value::Object(ValueMapping::from([
                (Name::new("accountAddress"), Value::String(erc20_balance.account_address)),
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
pub struct Erc20BalanceRaw {
    pub account_address: String,
    pub token_address: String,
    pub balance: String,
}

#[derive(FromRow, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Erc721BalanceRaw {
    pub account_address: String,
    pub token_address: String,
    pub token_id: String,
}

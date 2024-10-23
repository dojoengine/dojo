use async_graphql::dynamic::{Field, FieldFuture, InputValue, TypeRef};
use async_graphql::{Name, Value};
use convert_case::{Case, Casing};
use serde::Deserialize;
use sqlx::{FromRow, Pool, Sqlite, SqliteConnection};
use starknet_crypto::Felt;
use torii_core::engine::get_transaction_hash_from_event_id;
use torii_core::sql::utils::felt_to_sql_string;
use tracing::warn;

use crate::constants::{ERC_TRANSFER_NAME, ERC_TRANSFER_TYPE_NAME};
use crate::mapping::ERC_TRANSFER_TYPE_MAPPING;
use crate::object::{BasicObject, ResolvableObject};
use crate::types::{TypeMapping, ValueMapping};
use crate::utils::extract;

#[derive(Debug)]
pub struct ErcTransferObject;

impl BasicObject for ErcTransferObject {
    fn name(&self) -> (&str, &str) {
        ERC_TRANSFER_NAME
    }

    fn type_name(&self) -> &str {
        ERC_TRANSFER_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &ERC_TRANSFER_TYPE_MAPPING
    }
}

impl ResolvableObject for ErcTransferObject {
    fn resolvers(&self) -> Vec<Field> {
        let account_address = "account_address";
        let limit = "limit";
        let arg_addr = InputValue::new(
            account_address.to_case(Case::Camel),
            TypeRef::named_nn(TypeRef::STRING),
        );
        let arg_limit =
            InputValue::new(limit.to_case(Case::Camel), TypeRef::named_nn(TypeRef::INT));

        let field = Field::new(self.name().0, TypeRef::named_list(self.type_name()), move |ctx| {
            FieldFuture::new(async move {
                let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;
                let address = extract::<Felt>(
                    ctx.args.as_index_map(),
                    &account_address.to_case(Case::Camel),
                )?;
                let limit = extract::<u64>(ctx.args.as_index_map(), &limit.to_case(Case::Camel))?;
                let limit: u32 = limit.try_into()?;

                let erc_transfers = fetch_erc_transfers(&mut conn, address, limit).await?;

                Ok(Some(Value::List(erc_transfers)))
            })
        })
        .argument(arg_addr)
        .argument(arg_limit);
        vec![field]
    }
}

async fn fetch_erc_transfers(
    conn: &mut SqliteConnection,
    address: Felt,
    limit: u32,
) -> sqlx::Result<Vec<Value>> {
    let query = format!(
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
    c.contract_type
FROM
    erc_transfers et
JOIN
    tokens t ON et.token_id = t.id
JOIN
    contracts c ON t.contract_address = c.contract_address
WHERE
    et.from_address = ? OR et.to_address = ?
ORDER BY
    et.executed_at DESC
LIMIT {};
"#,
        limit
    );

    let address = felt_to_sql_string(&address);
    let rows = sqlx::query(&query).bind(&address).bind(&address).fetch_all(conn).await?;

    let mut erc_balances = Vec::new();

    for row in rows {
        let row = TransferQueryResultRaw::from_row(&row)?;
        let transaction_hash = get_transaction_hash_from_event_id(&row.id);

        let transfer_value = match row.contract_type.to_lowercase().as_str() {
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

                let token_metadata = Value::Object(ValueMapping::from([
                    (Name::new("name"), Value::String(row.name)),
                    (Name::new("symbol"), Value::String(row.symbol)),
                    (Name::new("tokenId"), Value::String(token_id[1].to_string())),
                    (Name::new("decimals"), Value::String(row.decimals.to_string())),
                    (Name::new("contractAddress"), Value::String(row.contract_address.clone())),
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

        erc_balances.push(transfer_value);
    }

    Ok(erc_balances)
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
}

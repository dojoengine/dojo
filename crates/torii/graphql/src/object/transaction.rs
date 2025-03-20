use std::str::FromStr;

use async_graphql::dynamic::{
    Field, FieldFuture, FieldValue, InputValue, SubscriptionField, SubscriptionFieldFuture, TypeRef,
};
use async_graphql::{Name, Value};
use sqlx::{FromRow, Pool, Sqlite};
use starknet_crypto::Felt;
use tokio_stream::StreamExt;
use torii_sqlite::simple_broker::SimpleBroker;
use torii_sqlite::types::Transaction;

use super::{BasicObject, ResolvableObject, TypeMapping, ValueMapping};
use crate::constants::{
    CALL_TYPE_NAME, ID_COLUMN, TOKEN_TRANSFER_TABLE, TOKEN_TRANSFER_TYPE_NAME,
    TRANSACTION_CALLS_TABLE, TRANSACTION_HASH_COLUMN, TRANSACTION_NAMES, TRANSACTION_TABLE,
    TRANSACTION_TYPE_NAME,
};
use crate::mapping::{CALL_MAPPING, TRANSACTION_MAPPING};
use crate::object::erc::token_transfer::{token_transfer_mapping_from_row, TransferQueryResultRaw};
use crate::object::{resolve_many, resolve_one};
use crate::query::value_mapping_from_row;
use crate::utils;

#[derive(Debug)]
pub struct CallObject;

impl BasicObject for CallObject {
    fn name(&self) -> (&str, &str) {
        ("call", "calls")
    }

    fn type_name(&self) -> &str {
        CALL_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &CALL_MAPPING
    }

    fn related_fields(&self) -> Option<Vec<Field>> {
        None
    }
}
#[derive(Debug)]
pub struct TransactionObject;

impl BasicObject for TransactionObject {
    fn name(&self) -> (&str, &str) {
        TRANSACTION_NAMES
    }

    fn type_name(&self) -> &str {
        TRANSACTION_TYPE_NAME
    }

    fn type_mapping(&self) -> &TypeMapping {
        &TRANSACTION_MAPPING
    }

    fn related_fields(&self) -> Option<Vec<Field>> {
        Some(vec![calls_field(), token_transfers_field()])
    }
}

impl ResolvableObject for TransactionObject {
    fn resolvers(&self) -> Vec<Field> {
        let resolve_one = resolve_one(
            TRANSACTION_TABLE,
            TRANSACTION_HASH_COLUMN,
            self.name().0,
            self.type_name(),
            self.type_mapping(),
        );

        let resolve_many = resolve_many(
            TRANSACTION_TABLE,
            ID_COLUMN,
            self.name().1,
            self.type_name(),
            self.type_mapping(),
        );

        vec![resolve_one, resolve_many]
    }

    fn subscriptions(&self) -> Option<Vec<SubscriptionField>> {
        Some(vec![
            SubscriptionField::new("transaction", TypeRef::named_nn(self.type_name()), |ctx| {
                SubscriptionFieldFuture::new(async move {
                    let hash = match ctx.args.get("hash") {
                        Some(hash) => Some(hash.string()?.to_string()),
                        None => None,
                    };

                    let caller = match ctx.args.get("hasCaller") {
                        Some(caller) => Some(caller.string()?.to_string()),
                        None => None,
                    };

                    // if hash is None, then subscribe to all transactions
                    // if hash is Some, then subscribe to only the transaction with that hash
                    Ok(SimpleBroker::<Transaction>::subscribe().filter_map(
                        move |transaction: Transaction| {
                            if (hash.is_none()
                                || hash == Some(transaction.transaction_hash.clone()))
                                && (caller.is_none()
                                    || transaction.calls.iter().any(|call| {
                                        call.caller_address
                                            == Felt::from_str(&caller.clone().unwrap()).unwrap()
                                    }))
                            {
                                Some(Ok(Value::Object(TransactionObject::value_mapping(
                                    transaction,
                                ))))
                            } else {
                                // hash != transaction.transaction_hash, then don't send anything,
                                // still listening
                                None
                            }
                        },
                    ))
                })
            })
            .argument(InputValue::new("hash", TypeRef::named(TypeRef::ID)))
            .argument(InputValue::new("hasCaller", TypeRef::named(TypeRef::STRING))),
        ])
    }
}

impl TransactionObject {
    pub fn value_mapping(transaction: Transaction) -> ValueMapping {
        async_graphql::dynamic::indexmap::IndexMap::from([
            (Name::new("id"), Value::from(transaction.id)),
            (Name::new("transactionHash"), Value::from(transaction.transaction_hash)),
            (Name::new("senderAddress"), Value::from(transaction.sender_address)),
            (
                Name::new("calldata"),
                Value::from(transaction.calldata.split("/").collect::<Vec<_>>()),
            ),
            (Name::new("maxFee"), Value::from(transaction.max_fee)),
            (Name::new("signature"), Value::from(transaction.signature)),
            (Name::new("nonce"), Value::from(transaction.nonce)),
            (Name::new("executedAt"), Value::from(transaction.executed_at.to_rfc3339())),
            (Name::new("createdAt"), Value::from(transaction.created_at.to_rfc3339())),
            (Name::new("transactionType"), Value::from(transaction.transaction_type)),
            (Name::new("blockNumber"), Value::from(transaction.block_number)),
        ])
    }
}

fn calls_field() -> Field {
    Field::new("calls", TypeRef::named_list(CALL_TYPE_NAME), move |ctx| {
        FieldFuture::new(async move {
            match ctx.parent_value.try_to_value()? {
                Value::Object(indexmap) => {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;

                    let transaction_hash = utils::extract::<String>(indexmap, "transactionHash")?;

                    // Fetch all function calls for this transaction
                    let query = &format!(
                        "SELECT * FROM {TRANSACTION_CALLS_TABLE} WHERE transaction_hash = ?"
                    );
                    let rows =
                        sqlx::query(query).bind(&transaction_hash).fetch_all(&mut *conn).await?;

                    let results = rows
                        .iter()
                        .map(|row| {
                            value_mapping_from_row(row, &CALL_MAPPING, false, true)
                                .map(Value::Object)
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    Ok(Some(Value::List(results)))
                }
                _ => Err("incorrect value, requires Value::Object".into()),
            }
        })
    })
}

fn token_transfers_field() -> Field {
    Field::new("tokenTransfers", TypeRef::named_list(TOKEN_TRANSFER_TYPE_NAME), move |ctx| {
        FieldFuture::new(async move {
            match ctx.parent_value.try_to_value()? {
                Value::Object(indexmap) => {
                    let mut conn = ctx.data::<Pool<Sqlite>>()?.acquire().await?;

                    let transaction_hash = utils::extract::<String>(indexmap, "transactionHash")?;

                    // Fetch all token transfers for this transaction
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
                            c.contract_type,
                            t.metadata
                        FROM
                            {TOKEN_TRANSFER_TABLE} et
                        JOIN
                            tokens t ON et.token_id = t.id
                        JOIN
                            contracts c ON t.contract_address = c.contract_address
                        WHERE
                            et.event_id LIKE '%:{transaction_hash}:%'
                        "#
                    );

                    let rows =
                        sqlx::query(&query).bind(&transaction_hash).fetch_all(&mut *conn).await?;

                    let mut results = Vec::new();
                    for row in &rows {
                        let row = TransferQueryResultRaw::from_row(row)?;
                        let result = token_transfer_mapping_from_row(&row)?;
                        results.push(FieldValue::owned_any(result));
                    }

                    Ok(Some(FieldValue::list(results)))
                }
                _ => Err("incorrect value, requires Value::Object".into()),
            }
        })
    })
}

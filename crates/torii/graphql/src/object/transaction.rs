use async_graphql::dynamic::{Field, FieldValue};
use async_graphql::dynamic::{FieldFuture, TypeRef};
use async_graphql::Value;
use sqlx::{FromRow, Pool, Sqlite};

use super::{BasicObject, ResolvableObject, TypeMapping};
use crate::constants::{
    CALL_TYPE_NAME, ID_COLUMN, TOKEN_TRANSFER_TABLE, TOKEN_TRANSFER_TYPE_NAME,
    TRANSACTION_CALLS_TABLE, TRANSACTION_HASH_COLUMN, TRANSACTION_NAMES, TRANSACTION_TABLE,
    TRANSACTION_TYPE_NAME,
};
use crate::mapping::{CALL_MAPPING, TRANSACTION_MAPPING};
use crate::object::erc::token_transfer::token_transfer_mapping_from_row;
use crate::object::erc::token_transfer::TransferQueryResultRaw;
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
                            value_mapping_from_row(&row, &CALL_MAPPING, false, true)
                                .map(|value| Value::Object(value))
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

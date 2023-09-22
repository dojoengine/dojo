use std::str::FromStr;

use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use starknet::core::types::{
    BlockWithTxs, FieldElement, InvokeTransaction, Transaction, TransactionReceipt,
};
use starknet::core::utils::parse_cairo_short_string;
use starknet::providers::Provider;

use super::TransactionProcessor;
use crate::sql::Sql;

#[derive(Default)]
pub struct StoreSystemCallProcessor;

const SYSTEM_NAME_OFFSET: usize = 6;
const ENTRYPOINT_OFFSET: usize = 2;
const EXECUTE_ENTRYPOINT: &str =
    "0x240060cdb34fcc260f41eac7474ee1d7c80b7e3607daff9ac67c7ea2ebb1c44";

#[async_trait]
impl<P: Provider + Sync> TransactionProcessor<P> for StoreSystemCallProcessor {
    async fn process(
        &self,
        db: &Sql,
        _provider: &P,
        block: &BlockWithTxs,
        transaction_receipt: &TransactionReceipt,
    ) -> Result<(), Error> {
        if let TransactionReceipt::Invoke(_) = transaction_receipt {
            for tx in &block.transactions {
                if let Some((tx_hash, system_name, calldata)) = parse_transaction(tx) {
                    let system_name = parse_cairo_short_string(&system_name)?;

                    db.store_system_call(system_name, tx_hash, calldata).await?;
                }
            }
        }

        Ok(())
    }
}

fn parse_transaction(
    transaction: &Transaction,
) -> Option<(FieldElement, FieldElement, &Vec<FieldElement>)> {
    if let Transaction::Invoke(InvokeTransaction::V1(tx)) = transaction {
        let entrypoint_felt = FieldElement::from_str(EXECUTE_ENTRYPOINT).unwrap();
        if tx.calldata[ENTRYPOINT_OFFSET] == entrypoint_felt {
            return Some((tx.transaction_hash, tx.calldata[SYSTEM_NAME_OFFSET], &tx.calldata));
        }
    }

    None
}

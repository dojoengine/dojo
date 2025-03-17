use std::collections::HashSet;

use anyhow::{Error, Ok, Result};
use async_trait::async_trait;
use starknet::core::types::{BlockId, BlockTag, ContractClass, Felt, InvokeTransaction, Transaction};
use starknet::macros::selector;
use starknet::providers::Provider;
use torii_sqlite::cache::{get_entrypoint_name_from_class, ContractClassCache};
use torii_sqlite::types::{CallType, ParsedCall};
use torii_sqlite::utils::felts_to_sql_string;
use torii_sqlite::Sql;

use super::TransactionProcessor;

#[derive(Default, Debug)]
pub struct StoreTransactionProcessor;

#[async_trait]
impl<P: Provider + Send + Sync + std::fmt::Debug> TransactionProcessor<P> for StoreTransactionProcessor {
    async fn process(
        &self,
        db: &mut Sql,
        _provider: &P,
        block_number: u64,
        block_timestamp: u64,
        _transaction_hash: Felt,
        contract_addresses: &HashSet<Felt>,
        transaction: &Transaction,
        contract_class_cache: &ContractClassCache<P>,
    ) -> Result<(), Error> {
        let transaction_type = match transaction {
            Transaction::Invoke(_) => "INVOKE",
            Transaction::L1Handler(_) => "L1_HANDLER",
            _ => return Ok(()),
        };

        let (transaction_hash, sender_address, calldata, max_fee, signature, nonce) =
            match transaction {
                Transaction::Invoke(InvokeTransaction::V3(invoke_v3_transaction)) => (
                    invoke_v3_transaction.transaction_hash,
                    invoke_v3_transaction.sender_address,
                    &invoke_v3_transaction.calldata,
                    Felt::ZERO, // has no max_fee
                    &invoke_v3_transaction.signature,
                    invoke_v3_transaction.nonce,
                ),
                Transaction::Invoke(InvokeTransaction::V1(invoke_v1_transaction)) => (
                    invoke_v1_transaction.transaction_hash,
                    invoke_v1_transaction.sender_address,
                    &invoke_v1_transaction.calldata,
                    invoke_v1_transaction.max_fee,
                    &invoke_v1_transaction.signature,
                    invoke_v1_transaction.nonce,
                ),
                Transaction::L1Handler(l1_handler_transaction) => (
                    l1_handler_transaction.transaction_hash,
                    l1_handler_transaction.contract_address,
                    &l1_handler_transaction.calldata,
                    Felt::ZERO,     // has no max_fee
                    &vec![], // has no signature
                    l1_handler_transaction.nonce.into(),
                ),
                _ => return Ok(()),
            };

        let mut calls: Vec<ParsedCall> = vec![];
        let calls_len: usize = calldata[0].try_into().unwrap();
        let mut offset = 0;
        for _ in 0..calls_len {
            let to_offset = offset + 1;
            let selector_offset = to_offset + 1;
            let calldata_offset = selector_offset + 2;
            let calldata_len: usize = calldata[selector_offset + 1].try_into().unwrap();
            let contract_address = calldata[to_offset];
            let contract_class =
                contract_class_cache.get(contract_address, BlockId::Number(block_number)).await?;
            let entrypoint =
                get_entrypoint_name_from_class(&contract_class, calldata[selector_offset])
                    .unwrap_or(format!("{:#x}", calldata[selector_offset]));

            let call = ParsedCall {
                contract_address,
                entrypoint,
                calldata: calldata[calldata_offset..calldata_offset + calldata_len].to_vec(),
                call_type: CallType::Execute,
                caller_address: sender_address,
            };

            if call.entrypoint == "execute_from_outside_v3" {
                let outside_calls_len: usize = calldata[calldata_offset + 5].try_into().unwrap();
                for _ in 0..outside_calls_len {
                    let to_offset = calldata_offset + 6;
                    let selector_offset = to_offset + 1;
                    let calldata_offset = selector_offset + 2;
                    let calldata_len: usize = calldata[selector_offset + 1].try_into().unwrap();
                    let contract_address = calldata[to_offset];
                    let contract_class = contract_class_cache
                        .get(contract_address, BlockId::Number(block_number))
                        .await?;
                    let entrypoint =
                        get_entrypoint_name_from_class(&contract_class, calldata[selector_offset])
                            .unwrap_or(format!("{:#x}", calldata[selector_offset]));

                    let outside_call = ParsedCall {
                        contract_address,
                        entrypoint,
                        calldata: calldata[calldata_offset..calldata_offset + calldata_len]
                            .to_vec(),
                        call_type: CallType::ExecuteFromOutside,
                        caller_address: call.contract_address,
                    };
                    calls.push(outside_call);
                }
            } else if call.entrypoint == "execute_from_outside_v2" {
                // the execute_from_outside_v2 nonce is only a felt, thus we have a 4 offset
                let outside_calls_len: usize = calldata[calldata_offset + 4].try_into().unwrap();
                for _ in 0..outside_calls_len {
                    let to_offset = calldata_offset + 5;
                    let selector_offset = to_offset + 1;
                    let calldata_offset = selector_offset + 2;
                    let calldata_len: usize = calldata[selector_offset + 1].try_into().unwrap();
                    let contract_address = calldata[to_offset];
                    let contract_class = contract_class_cache
                        .get(contract_address, BlockId::Number(block_number))
                        .await?;
                    let entrypoint =
                        get_entrypoint_name_from_class(&contract_class, calldata[selector_offset])
                            .unwrap_or(format!("{:#x}", calldata[selector_offset]));

                    let outside_call = ParsedCall {
                        contract_address,
                        entrypoint,
                        calldata: calldata[calldata_offset..calldata_offset + calldata_len]
                            .to_vec(),
                        call_type: CallType::ExecuteFromOutside,
                        caller_address: call.contract_address,
                    };
                    calls.push(outside_call);
                }
            }

            calls.push(call);
            offset += 3 + calldata_len;
        }

        db.store_transaction(
            transaction_hash,
            sender_address,
            calldata,
            max_fee,
            signature,
            nonce,
            block_number,
            contract_addresses,
            transaction_type,
            block_timestamp,
            &calls,
        )?;
        Ok(())
    }
}

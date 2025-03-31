use std::collections::HashSet;

use anyhow::Result;
use async_trait::async_trait;
use cainome::cairo_serde_derive::CairoSerde;
use cainome_cairo_serde::CairoSerde;
use starknet::core::types::{BlockId, BlockTag, Felt, InvokeTransaction, Transaction};
use starknet::providers::Provider;
use torii_sqlite::cache::{get_entrypoint_name_from_class, ContractClassCache};
use torii_sqlite::types::{CallType, ParsedCall};
use torii_sqlite::Sql;

use super::TransactionProcessor;

#[derive(CairoSerde, Debug, Clone)]
pub struct ExecuteCall {
    pub contract_address: Felt,
    pub selector: Felt,
    pub calldata: Vec<Felt>,
}

#[derive(CairoSerde, Debug, Clone)]
pub struct LegacyExecuteCall {
    pub contract_address: Felt,
    pub selector: Felt,
    pub data_offset: usize,
    pub data_length: usize,
}

#[derive(CairoSerde, Debug, Clone)]
pub struct ExecuteTransaction {
    pub calls: Vec<ExecuteCall>,
}

#[derive(CairoSerde, Debug, Clone)]
pub struct LegacyExecuteTransaction {
    pub calls: Vec<LegacyExecuteCall>,
    pub calldata: Vec<Felt>,
}

#[derive(CairoSerde, Debug, Clone)]
pub enum Execute {
    Legacy(LegacyExecuteTransaction),
    Execute(ExecuteTransaction),
}

struct TransactionInfo {
    transaction_hash: Felt,
    sender_address: Felt,
    calldata: Vec<Felt>,
    max_fee: Felt,
    signature: Vec<Felt>,
    nonce: Felt,
    transaction_type: &'static str,
}

#[derive(Default, Debug)]
pub struct StoreTransactionProcessor;

impl StoreTransactionProcessor {
    fn extract_transaction_info(transaction: &Transaction) -> Option<TransactionInfo> {
        match transaction {
            Transaction::Invoke(InvokeTransaction::V3(tx)) => Some(TransactionInfo {
                transaction_hash: tx.transaction_hash,
                sender_address: tx.sender_address,
                calldata: tx.calldata.clone(),
                max_fee: Felt::ZERO,
                signature: tx.signature.clone(),
                nonce: tx.nonce,
                transaction_type: "INVOKE",
            }),
            Transaction::Invoke(InvokeTransaction::V1(tx)) => Some(TransactionInfo {
                transaction_hash: tx.transaction_hash,
                sender_address: tx.sender_address,
                calldata: tx.calldata.clone(),
                max_fee: tx.max_fee,
                signature: tx.signature.clone(),
                nonce: tx.nonce,
                transaction_type: "INVOKE",
            }),
            Transaction::L1Handler(tx) => Some(TransactionInfo {
                transaction_hash: tx.transaction_hash,
                sender_address: tx.contract_address,
                calldata: tx.calldata.clone(),
                max_fee: Felt::ZERO,
                signature: vec![],
                nonce: tx.nonce.into(),
                transaction_type: "L1_HANDLER",
            }),
            _ => None,
        }
    }

    async fn parse_execute_call<P: Provider + Send + Sync + std::fmt::Debug>(
        contract_class_cache: &ContractClassCache<P>,
        call: &ExecuteCall,
        caller_address: Felt,
        call_type: CallType,
    ) -> Result<ParsedCall> {
        let contract_class = contract_class_cache
            .get(call.contract_address, BlockId::Tag(BlockTag::Pending))
            .await?;

        let entrypoint = get_entrypoint_name_from_class(&contract_class, call.selector)
            .unwrap_or(format!("{:#x}", call.selector));

        Ok(ParsedCall {
            contract_address: call.contract_address,
            entrypoint,
            calldata: call.calldata.clone(),
            call_type,
            caller_address,
        })
    }

    async fn parse_legacy_execute_call<P: Provider + Send + Sync + std::fmt::Debug>(
        contract_class_cache: &ContractClassCache<P>,
        call: &LegacyExecuteCall,
        full_calldata: &[Felt],
        caller_address: Felt,
        call_type: CallType,
    ) -> Result<ParsedCall> {
        let contract_class = contract_class_cache
            .get(call.contract_address, BlockId::Tag(BlockTag::Pending))
            .await?;

        let entrypoint = get_entrypoint_name_from_class(&contract_class, call.selector)
            .unwrap_or(format!("{:#x}", call.selector));

        Ok(ParsedCall {
            contract_address: call.contract_address,
            entrypoint,
            calldata: full_calldata[call.data_offset..call.data_offset + call.data_length].to_vec(),
            call_type,
            caller_address,
        })
    }

    async fn parse_outside_call<P: Provider + Send + Sync + std::fmt::Debug>(
        contract_class_cache: &ContractClassCache<P>,
        calldata: &[Felt],
        base_offset: usize,
        caller_address: Felt,
    ) -> Result<ParsedCall> {
        let to_offset = base_offset;
        let selector_offset = to_offset + 1;
        let calldata_offset = selector_offset + 2;
        let calldata_len: usize = calldata[selector_offset + 1].try_into().unwrap();
        let contract_address = calldata[to_offset];

        let contract_class =
            contract_class_cache.get(contract_address, BlockId::Tag(BlockTag::Pending)).await?;

        let entrypoint = get_entrypoint_name_from_class(&contract_class, calldata[selector_offset])
            .unwrap_or(format!("{:#x}", calldata[selector_offset]));

        Ok(ParsedCall {
            contract_address,
            entrypoint,
            calldata: calldata[calldata_offset..calldata_offset + calldata_len].to_vec(),
            call_type: CallType::ExecuteFromOutside,
            caller_address,
        })
    }

    async fn process_outside_calls<P: Provider + Send + Sync + std::fmt::Debug>(
        contract_class_cache: &ContractClassCache<P>,
        call: &ParsedCall,
    ) -> Result<Vec<ParsedCall>> {
        let mut outside_calls = Vec::new();

        match call.entrypoint.as_str() {
            "execute_from_outside_v3" => {
                let outside_calls_len: usize = call.calldata[5].try_into().unwrap();
                for _ in 0..outside_calls_len {
                    let outside_call = Self::parse_outside_call(
                        contract_class_cache,
                        &call.calldata,
                        6,
                        call.contract_address,
                    )
                    .await?;
                    outside_calls.push(outside_call);
                }
            }
            "execute_from_outside_v2" => {
                let outside_calls_len: usize = call.calldata[4].try_into().unwrap();
                for _ in 0..outside_calls_len {
                    let outside_call = Self::parse_outside_call(
                        contract_class_cache,
                        &call.calldata,
                        5,
                        call.contract_address,
                    )
                    .await?;
                    outside_calls.push(outside_call);
                }
            }
            _ => {}
        }

        Ok(outside_calls)
    }

    async fn parse_execute<P: Provider + Send + Sync + std::fmt::Debug>(
        execute: Execute,
        sender_address: Felt,
        contract_class_cache: &ContractClassCache<P>,
    ) -> Result<Vec<ParsedCall>> {
        let mut calls = Vec::new();

        match execute {
            Execute::Execute(execute) => {
                for call in execute.calls {
                    let parsed_call = Self::parse_execute_call(
                        contract_class_cache,
                        &call,
                        sender_address,
                        CallType::Execute,
                    )
                    .await?;
                    calls.push(parsed_call);
                }
            }
            Execute::Legacy(execute) => {
                for call in execute.calls {
                    let parsed_call = Self::parse_legacy_execute_call(
                        contract_class_cache,
                        &call,
                        &execute.calldata,
                        sender_address,
                        CallType::Execute,
                    )
                    .await?;
                    calls.push(parsed_call);
                }
            }
        }

        // Process any outside calls
        let mut all_calls = calls.clone();
        for call in calls {
            let mut outside_calls =
                Self::process_outside_calls(contract_class_cache, &call).await?;
            all_calls.append(&mut outside_calls);
        }

        Ok(all_calls)
    }
}

#[async_trait]
impl<P: Provider + Send + Sync + std::fmt::Debug> TransactionProcessor<P>
    for StoreTransactionProcessor
{
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
    ) -> Result<()> {
        let Some(tx_info) = Self::extract_transaction_info(transaction) else {
            return Ok(());
        };

        let calls = if tx_info.transaction_type == "INVOKE" {
            let execute =
                if let Ok(execute) = ExecuteTransaction::cairo_deserialize(&tx_info.calldata, 0) {
                    Some(Execute::Execute(execute))
                } else if let Ok(execute) =
                    LegacyExecuteTransaction::cairo_deserialize(&tx_info.calldata, 0)
                {
                    Some(Execute::Legacy(execute))
                } else {
                    None
                };

            if let Some(execute) = execute {
                Self::parse_execute(execute, tx_info.sender_address, contract_class_cache).await?
            } else {
                vec![]
            }
        } else {
            vec![]
        };

        db.store_transaction(
            tx_info.transaction_hash,
            tx_info.sender_address,
            &tx_info.calldata,
            tx_info.max_fee,
            &tx_info.signature,
            tx_info.nonce,
            block_number,
            contract_addresses,
            tx_info.transaction_type,
            block_timestamp,
            &calls,
        )?;

        Ok(())
    }
}

use std::collections::HashMap;

use blockifier::state::cached_state::CommitmentStateDiff;
use blockifier::transaction::objects::TransactionExecutionInfo;
use katana_primitives::contract::{ClassHash, CompiledContractClass, ContractAddress, SierraClass};
use katana_primitives::transaction::{Receipt, Tx};

use super::utils::{events_from_exec_info, l2_to_l1_messages_from_exec_info};

pub struct ExecutedTx {
    pub tx: Tx,
    pub receipt: Receipt,
    pub execution_info: TransactionExecutionInfo,
}

impl ExecutedTx {
    pub(super) fn new(tx: Tx, execution_info: TransactionExecutionInfo) -> Self {
        let actual_fee = execution_info.actual_fee.0;
        let events = events_from_exec_info(&execution_info);
        let revert_error = execution_info.revert_error.clone();
        let messages_sent = l2_to_l1_messages_from_exec_info(&execution_info);
        let actual_resources = execution_info.actual_resources.0.clone();

        let contract_address = if let Tx::DeployAccount(ref tx) = tx {
            Some(ContractAddress(tx.contract_address.into()))
        } else {
            None
        };

        Self {
            tx,
            execution_info,
            receipt: Receipt {
                events,
                actual_fee,
                revert_error,
                messages_sent,
                actual_resources,
                contract_address,
            },
        }
    }
}

/// The outcome that after executing a list of transactions.
pub struct ExecutionOutcome {
    pub transactions: Vec<ExecutedTx>,
    pub state_diff: CommitmentStateDiff,
    pub declared_classes: HashMap<ClassHash, CompiledContractClass>,
    pub declared_sierra_classes: HashMap<ClassHash, SierraClass>,
}

impl Default for ExecutionOutcome {
    fn default() -> Self {
        let state_diff = CommitmentStateDiff {
            storage_updates: Default::default(),
            address_to_nonce: Default::default(),
            address_to_class_hash: Default::default(),
            class_hash_to_compiled_class_hash: Default::default(),
        };

        Self {
            state_diff,
            transactions: Default::default(),
            declared_classes: Default::default(),
            declared_sierra_classes: Default::default(),
        }
    }
}

use starknet::providers::sequencer::models::{
    Block as FgwBlock, BlockStatus, InvokeFunctionTransaction, StateUpdate as FgwStateUpdate,
};

use crate::block::{FinalityStatus, GasPrices, Header, SealedBlock, SealedBlockWithStatus};
use crate::da::L1DataAvailabilityMode;
use crate::state::StateUpdates;
use crate::transaction::TxWithHash;
use crate::ContractAddress;

impl From<FgwBlock> for SealedBlockWithStatus {
    fn from(value: FgwBlock) -> Self {
        let block = SealedBlock {
            body: Vec::new(),
            hash: value.block_hash.unwrap_or_default().into(),
            header: Header {
                parent_hash: value.parent_block_hash.into(),
                number: value.block_number.unwrap_or_default(),
                state_diff_commitment: Default::default(),
                transactions_commitment: value.transaction_commitment.unwrap_or_default(),
                receipts_commitment: Default::default(),
                events_commitment: value.event_commitment.unwrap_or_default(),
                state_root: value.state_root.unwrap_or_default(),
                transaction_count: value.transactions.len() as u32,
                events_count: Default::default(),
                state_diff_length: Default::default(),
                timestamp: value.timestamp,
                sequencer_address: value.sequencer_address.unwrap_or_default().into(),
                l1_gas_prices: GasPrices::default(),
                l1_data_gas_prices: GasPrices::default(),
                l1_da_mode: L1DataAvailabilityMode::Calldata,
                protocol_version: Default::default(),
            },
        };

        let status = match value.status {
            BlockStatus::AcceptedOnL2 => FinalityStatus::AcceptedOnL2,
            BlockStatus::AcceptedOnL1 => FinalityStatus::AcceptedOnL1,
            status => panic!("unsupported block status: {status:?}"),
        };

        SealedBlockWithStatus { block, status }
    }
}

impl From<FgwStateUpdate> for StateUpdates {
    fn from(value: FgwStateUpdate) -> Self {
        let state_diff = value.state_diff;

        let nonce_updates = state_diff
            .nonces
            .into_iter()
            .map(|(addr, nonce)| (ContractAddress::from(addr), nonce))
            .collect();

        let storage_updates = state_diff
            .storage_diffs
            .into_iter()
            .map(|(addr, diffs)| {
                let storage_map = diffs.into_iter().map(|diff| (diff.key, diff.value)).collect();
                (ContractAddress::from(addr), storage_map)
            })
            .collect();

        let deployed_contracts = state_diff
            .deployed_contracts
            .into_iter()
            .map(|contract| (ContractAddress::from(contract.address), contract.class_hash))
            .collect();

        let declared_classes = state_diff
            .declared_classes
            .into_iter()
            .map(|contract| (contract.class_hash.into(), contract.compiled_class_hash))
            .collect();

        let deprecated_declared_classes = state_diff.old_declared_contracts.into_iter().collect();

        let replaced_classes = state_diff
            .replaced_classes
            .into_iter()
            .map(|contract| (ContractAddress::from(contract.address), contract.class_hash))
            .collect();

        StateUpdates {
            nonce_updates,
            storage_updates,
            declared_classes,
            replaced_classes,
            deployed_contracts,
            deprecated_declared_classes,
        }
    }
}

impl From<InvokeFunctionTransaction> for TxWithHash {
    fn from(value: InvokeFunctionTransaction) -> Self {
        todo!()
    }
}

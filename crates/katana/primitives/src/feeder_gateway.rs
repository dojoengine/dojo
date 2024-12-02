use num_traits::ToPrimitive;
use starknet::core::types::ResourcePrice;
use starknet::providers::sequencer::models::{
    Block as FgwBlock, BlockStatus, InvokeFunctionTransaction, StateUpdate as FgwStateUpdate,
};

use crate::block::{FinalityStatus, GasPrices, Header, SealedBlock, SealedBlockWithStatus};
use crate::da::{DataAvailabilityMode, L1DataAvailabilityMode};
use crate::state::StateUpdates;
use crate::transaction::{InvokeTx, InvokeTxV1, InvokeTxV3, Tx, TxWithHash};
use crate::version::ProtocolVersion;
use crate::{ContractAddress, Felt};

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
                l1_gas_prices: value.l1_gas_price.into(),
                l1_data_gas_prices: value.l1_data_gas_price.into(),
                l1_da_mode: L1DataAvailabilityMode::Calldata,
                // old blocks dont include the version field
                protocol_version: value
                    .starknet_version
                    .and_then(|v| ProtocolVersion::parse(&v).ok())
                    .unwrap_or_default(),
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

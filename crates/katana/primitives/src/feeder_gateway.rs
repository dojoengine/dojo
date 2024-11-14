use starknet::providers::sequencer::models::Block;

use crate::block::{Header, SealedBlock};

impl TryFrom<Block> for SealedBlock {
    type Error = std::convert::Infallible;

    fn try_from(value: Block) -> Result<Self, Self::Error> {
        Ok(SealedBlock {
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
                l1_gas_prices: value.l1_gas_price,
                l1_data_gas_prices: value.l1_data_gas_price,
                l1_da_mode: value.l1_da_mode,
                protocol_version: Default::default(),
            },
            body: value.transactions.into_iter().map(|tx| tx.into()).collect(),
        })
    }
}

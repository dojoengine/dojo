use katana_primitives::block::{Block, BlockHash, BlockNumber, FinalityStatus, PartialHeader};
use katana_primitives::transaction::{TxHash, TxWithHash};
use serde::Serialize;
use starknet::core::types::{BlockStatus, ResourcePrice};

pub type BlockTxCount = u64;

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct BlockWithTxs(starknet::core::types::BlockWithTxs);

impl BlockWithTxs {
    pub fn new(block_hash: BlockHash, block: Block, finality_status: FinalityStatus) -> Self {
        let l1_gas_price = ResourcePrice {
            price_in_wei: block.header.gas_prices.eth,
            price_in_strk: Some(block.header.gas_prices.strk),
        };

        let transactions =
            block.body.into_iter().map(|tx| crate::transaction::Tx::from(tx).0).collect();

        Self(starknet::core::types::BlockWithTxs {
            block_hash,
            l1_gas_price,
            transactions,
            new_root: block.header.state_root,
            timestamp: block.header.timestamp,
            block_number: block.header.number,
            parent_hash: block.header.parent_hash,
            starknet_version: block.header.version.to_string(),
            sequencer_address: block.header.sequencer_address.into(),
            status: match finality_status {
                FinalityStatus::AcceptedOnL1 => BlockStatus::AcceptedOnL1,
                FinalityStatus::AcceptedOnL2 => BlockStatus::AcceptedOnL2,
            },
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct PendingBlockWithTxs(starknet::core::types::PendingBlockWithTxs);

impl PendingBlockWithTxs {
    pub fn new(header: PartialHeader, transactions: Vec<TxWithHash>) -> Self {
        let transactions =
            transactions.into_iter().map(|tx| crate::transaction::Tx::from(tx).0).collect();

        let l1_gas_price = ResourcePrice {
            price_in_wei: header.gas_prices.eth,
            price_in_strk: Some(header.gas_prices.strk),
        };

        Self(starknet::core::types::PendingBlockWithTxs {
            transactions,
            l1_gas_price,
            timestamp: header.timestamp,
            parent_hash: header.parent_hash,
            starknet_version: header.version.to_string(),
            sequencer_address: header.sequencer_address.into(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum MaybePendingBlockWithTxs {
    Pending(PendingBlockWithTxs),
    Block(BlockWithTxs),
}

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct BlockWithTxHashes(starknet::core::types::BlockWithTxHashes);

impl BlockWithTxHashes {
    pub fn new(
        block_hash: BlockHash,
        block: katana_primitives::block::BlockWithTxHashes,
        finality_status: FinalityStatus,
    ) -> Self {
        let l1_gas_price = ResourcePrice {
            price_in_wei: block.header.gas_prices.eth,
            price_in_strk: Some(block.header.gas_prices.strk),
        };

        Self(starknet::core::types::BlockWithTxHashes {
            block_hash,
            l1_gas_price,
            transactions: block.body,
            new_root: block.header.state_root,
            timestamp: block.header.timestamp,
            block_number: block.header.number,
            parent_hash: block.header.parent_hash,
            starknet_version: block.header.version.to_string(),
            sequencer_address: block.header.sequencer_address.into(),
            status: match finality_status {
                FinalityStatus::AcceptedOnL1 => BlockStatus::AcceptedOnL1,
                FinalityStatus::AcceptedOnL2 => BlockStatus::AcceptedOnL2,
            },
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct PendingBlockWithTxHashes(starknet::core::types::PendingBlockWithTxHashes);

impl PendingBlockWithTxHashes {
    pub fn new(header: PartialHeader, transactions: Vec<TxHash>) -> Self {
        let l1_gas_price = ResourcePrice {
            price_in_wei: header.gas_prices.eth,
            price_in_strk: Some(header.gas_prices.strk),
        };

        Self(starknet::core::types::PendingBlockWithTxHashes {
            transactions,
            l1_gas_price,
            timestamp: header.timestamp,
            parent_hash: header.parent_hash,
            starknet_version: header.version.to_string(),
            sequencer_address: header.sequencer_address.into(),
        })
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum MaybePendingBlockWithTxHashes {
    Pending(PendingBlockWithTxHashes),
    Block(BlockWithTxHashes),
}

#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub struct BlockHashAndNumber(starknet::core::types::BlockHashAndNumber);

impl BlockHashAndNumber {
    pub fn new(hash: BlockHash, number: BlockNumber) -> Self {
        Self(starknet::core::types::BlockHashAndNumber { block_hash: hash, block_number: number })
    }
}

impl From<(BlockHash, BlockNumber)> for BlockHashAndNumber {
    fn from((hash, number): (BlockHash, BlockNumber)) -> Self {
        Self::new(hash, number)
    }
}

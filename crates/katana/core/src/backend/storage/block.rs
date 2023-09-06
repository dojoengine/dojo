use std::sync::Arc;

use starknet::core::crypto::compute_hash_on_elements;
use starknet::core::types::{
    BlockStatus as RpcBlockStatus, BlockWithTxHashes, BlockWithTxs, FieldElement,
    MaybePendingBlockWithTxHashes, MaybePendingBlockWithTxs, PendingBlockWithTxHashes,
    PendingBlockWithTxs,
};

use super::transaction::TransactionOutput;
use crate::execution::ExecutedTransaction;
use crate::utils::transaction::api_to_rpc_transaction;

#[derive(Debug, Clone, Copy)]
pub enum BlockStatus {
    Rejected,
    AcceptedOnL2,
    AcceptedOnL1,
}

#[derive(Debug, Clone)]
pub struct PartialHeader {
    pub parent_hash: FieldElement,
    pub number: u64,
    pub gas_price: u128,
    pub timestamp: u64,
    pub sequencer_address: FieldElement,
}

#[derive(Debug, Clone)]
pub struct Header {
    pub parent_hash: FieldElement,
    pub number: u64,
    pub gas_price: u128,
    pub timestamp: u64,
    pub state_root: FieldElement,
    pub sequencer_address: FieldElement,
}

impl Header {
    pub fn new(partial_header: PartialHeader, state_root: FieldElement) -> Self {
        Self {
            state_root,
            number: partial_header.number,
            gas_price: partial_header.gas_price,
            timestamp: partial_header.timestamp,
            parent_hash: partial_header.parent_hash,
            sequencer_address: partial_header.sequencer_address,
        }
    }

    pub fn hash(&self) -> FieldElement {
        compute_hash_on_elements(&vec![
            self.number.into(),     // block number
            self.state_root,        // state root
            self.sequencer_address, // sequencer address
            self.timestamp.into(),  // block timestamp
            FieldElement::ZERO,     // transaction commitment
            FieldElement::ZERO,     // event commitment
            FieldElement::ZERO,     // protocol version
            FieldElement::ZERO,     // extra data
            self.parent_hash,       // parent hash
        ])
    }
}

#[derive(Debug, Clone)]
pub enum ExecutedBlock {
    Pending(PartialBlock),
    Included(Block),
}

impl ExecutedBlock {
    pub fn transaction_count(&self) -> usize {
        match self {
            Self::Pending(block) => block.transactions.len(),
            Self::Included(block) => block.transactions.len(),
        }
    }

    pub fn transactions(&self) -> &[Arc<ExecutedTransaction>] {
        match self {
            Self::Pending(block) => &block.transactions,
            Self::Included(block) => &block.transactions,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PartialBlock {
    pub header: PartialHeader,
    pub transactions: Vec<Arc<ExecutedTransaction>>,
    pub outputs: Vec<TransactionOutput>,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub header: Header,
    pub status: BlockStatus,
    pub transactions: Vec<Arc<ExecutedTransaction>>,
    pub outputs: Vec<TransactionOutput>,
}

impl Block {
    pub fn new(
        partial_header: PartialHeader,
        transactions: Vec<Arc<ExecutedTransaction>>,
        outputs: Vec<TransactionOutput>,
    ) -> Self {
        // TODO: compute state root
        let state_root = FieldElement::ZERO;

        Self {
            header: Header::new(partial_header, state_root),
            status: BlockStatus::AcceptedOnL2,
            transactions,
            outputs,
        }
    }
}

impl From<PartialBlock> for ExecutedBlock {
    fn from(value: PartialBlock) -> Self {
        Self::Pending(value)
    }
}

impl From<Block> for ExecutedBlock {
    fn from(value: Block) -> Self {
        Self::Included(value)
    }
}

impl From<BlockStatus> for RpcBlockStatus {
    fn from(value: BlockStatus) -> Self {
        match value {
            BlockStatus::Rejected => Self::Rejected,
            BlockStatus::AcceptedOnL2 => Self::AcceptedOnL2,
            BlockStatus::AcceptedOnL1 => Self::AcceptedOnL1,
        }
    }
}

impl From<Block> for BlockWithTxs {
    fn from(value: Block) -> Self {
        Self {
            status: value.status.into(),
            block_hash: value.header.hash(),
            block_number: value.header.number,
            new_root: value.header.state_root,
            timestamp: value.header.timestamp,
            parent_hash: value.header.parent_hash,
            sequencer_address: value.header.sequencer_address,
            transactions: value
                .transactions
                .into_iter()
                .map(|t| api_to_rpc_transaction(t.inner.clone().into()))
                .collect(),
        }
    }
}

impl From<ExecutedBlock> for MaybePendingBlockWithTxs {
    fn from(value: ExecutedBlock) -> Self {
        match value {
            ExecutedBlock::Included(block) => MaybePendingBlockWithTxs::Block(BlockWithTxs {
                status: block.status.into(),
                block_hash: block.header.hash(),
                block_number: block.header.number,
                new_root: block.header.state_root,
                timestamp: block.header.timestamp,
                parent_hash: block.header.parent_hash,
                sequencer_address: block.header.sequencer_address,
                transactions: block
                    .transactions
                    .into_iter()
                    .map(|t| api_to_rpc_transaction(t.inner.clone().into()))
                    .collect(),
            }),
            ExecutedBlock::Pending(block) => {
                MaybePendingBlockWithTxs::PendingBlock(PendingBlockWithTxs {
                    timestamp: block.header.timestamp,
                    parent_hash: block.header.parent_hash,
                    sequencer_address: block.header.sequencer_address,
                    transactions: block
                        .transactions
                        .into_iter()
                        .map(|t| api_to_rpc_transaction(t.inner.clone().into()))
                        .collect(),
                })
            }
        }
    }
}

impl From<ExecutedBlock> for MaybePendingBlockWithTxHashes {
    fn from(value: ExecutedBlock) -> Self {
        match value {
            ExecutedBlock::Included(block) => {
                MaybePendingBlockWithTxHashes::Block(BlockWithTxHashes {
                    status: block.status.into(),
                    block_hash: block.header.hash(),
                    block_number: block.header.number,
                    new_root: block.header.state_root,
                    timestamp: block.header.timestamp,
                    parent_hash: block.header.parent_hash,
                    sequencer_address: block.header.sequencer_address,
                    transactions: block.transactions.into_iter().map(|t| t.inner.hash()).collect(),
                })
            }
            ExecutedBlock::Pending(block) => {
                MaybePendingBlockWithTxHashes::PendingBlock(PendingBlockWithTxHashes {
                    timestamp: block.header.timestamp,
                    parent_hash: block.header.parent_hash,
                    sequencer_address: block.header.sequencer_address,
                    transactions: block.transactions.into_iter().map(|t| t.inner.hash()).collect(),
                })
            }
        }
    }
}

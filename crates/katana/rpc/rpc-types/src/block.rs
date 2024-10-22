use katana_primitives::block::{
    Block, BlockHash, BlockNumber, FinalityStatus, Header, PartialHeader,
};
use katana_primitives::receipt::Receipt;
use katana_primitives::transaction::{TxHash, TxWithHash};
use serde::{Deserialize, Serialize};
use starknet::core::types::{
    BlockStatus, L1DataAvailabilityMode, ResourcePrice, TransactionWithReceipt,
};

use crate::receipt::TxReceipt;
use crate::transaction::Tx;

pub type BlockTxCount = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockWithTxs(starknet::core::types::BlockWithTxs);

impl BlockWithTxs {
    pub fn new(block_hash: BlockHash, block: Block, finality_status: FinalityStatus) -> Self {
        let l1_gas_price = ResourcePrice {
            price_in_wei: block.header.l1_gas_prices.eth.into(),
            price_in_fri: block.header.l1_gas_prices.strk.into(),
        };

        let l1_data_gas_price = ResourcePrice {
            price_in_wei: block.header.l1_data_gas_prices.eth.into(),
            price_in_fri: block.header.l1_data_gas_prices.strk.into(),
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
            starknet_version: block.header.protocol_version.to_string(),
            sequencer_address: block.header.sequencer_address.into(),
            status: match finality_status {
                FinalityStatus::AcceptedOnL1 => BlockStatus::AcceptedOnL1,
                FinalityStatus::AcceptedOnL2 => BlockStatus::AcceptedOnL2,
            },
            l1_da_mode: match block.header.l1_da_mode {
                katana_primitives::da::L1DataAvailabilityMode::Blob => L1DataAvailabilityMode::Blob,
                katana_primitives::da::L1DataAvailabilityMode::Calldata => {
                    L1DataAvailabilityMode::Calldata
                }
            },
            l1_data_gas_price,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PendingBlockWithTxs(starknet::core::types::PendingBlockWithTxs);

impl PendingBlockWithTxs {
    pub fn new(header: PartialHeader, transactions: Vec<TxWithHash>) -> Self {
        let transactions =
            transactions.into_iter().map(|tx| crate::transaction::Tx::from(tx).0).collect();

        let l1_gas_price = ResourcePrice {
            price_in_wei: header.l1_gas_prices.eth.into(),
            price_in_fri: header.l1_gas_prices.strk.into(),
        };

        Self(starknet::core::types::PendingBlockWithTxs {
            transactions,
            l1_gas_price,
            timestamp: header.timestamp,
            parent_hash: header.parent_hash,
            starknet_version: header.protocol_version.to_string(),
            sequencer_address: header.sequencer_address.into(),

            l1_da_mode: L1DataAvailabilityMode::Calldata,
            l1_data_gas_price: ResourcePrice {
                price_in_fri: Default::default(),
                price_in_wei: Default::default(),
            },
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MaybePendingBlockWithTxs {
    Pending(PendingBlockWithTxs),
    Block(BlockWithTxs),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockWithTxHashes(starknet::core::types::BlockWithTxHashes);

impl BlockWithTxHashes {
    pub fn new(
        block_hash: BlockHash,
        block: katana_primitives::block::BlockWithTxHashes,
        finality_status: FinalityStatus,
    ) -> Self {
        let l1_gas_price = ResourcePrice {
            price_in_wei: block.header.l1_gas_prices.eth.into(),
            price_in_fri: block.header.l1_gas_prices.strk.into(),
        };

        let l1_data_gas_price = ResourcePrice {
            price_in_wei: block.header.l1_data_gas_prices.eth.into(),
            price_in_fri: block.header.l1_data_gas_prices.strk.into(),
        };

        Self(starknet::core::types::BlockWithTxHashes {
            block_hash,
            l1_gas_price,
            transactions: block.body,
            new_root: block.header.state_root,
            timestamp: block.header.timestamp,
            block_number: block.header.number,
            parent_hash: block.header.parent_hash,
            starknet_version: block.header.protocol_version.to_string(),
            sequencer_address: block.header.sequencer_address.into(),
            status: match finality_status {
                FinalityStatus::AcceptedOnL1 => BlockStatus::AcceptedOnL1,
                FinalityStatus::AcceptedOnL2 => BlockStatus::AcceptedOnL2,
            },
            l1_da_mode: match block.header.l1_da_mode {
                katana_primitives::da::L1DataAvailabilityMode::Blob => L1DataAvailabilityMode::Blob,
                katana_primitives::da::L1DataAvailabilityMode::Calldata => {
                    L1DataAvailabilityMode::Calldata
                }
            },
            l1_data_gas_price,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PendingBlockWithTxHashes(starknet::core::types::PendingBlockWithTxHashes);

impl PendingBlockWithTxHashes {
    pub fn new(header: PartialHeader, transactions: Vec<TxHash>) -> Self {
        let l1_gas_price = ResourcePrice {
            price_in_wei: header.l1_gas_prices.eth.into(),
            price_in_fri: header.l1_gas_prices.strk.into(),
        };

        let l1_data_gas_price = ResourcePrice {
            price_in_wei: header.l1_data_gas_prices.eth.into(),
            price_in_fri: header.l1_data_gas_prices.strk.into(),
        };

        Self(starknet::core::types::PendingBlockWithTxHashes {
            transactions,
            l1_gas_price,
            timestamp: header.timestamp,
            parent_hash: header.parent_hash,
            starknet_version: header.protocol_version.to_string(),
            sequencer_address: header.sequencer_address.into(),
            l1_da_mode: match header.l1_da_mode {
                katana_primitives::da::L1DataAvailabilityMode::Blob => L1DataAvailabilityMode::Blob,
                katana_primitives::da::L1DataAvailabilityMode::Calldata => {
                    L1DataAvailabilityMode::Calldata
                }
            },
            l1_data_gas_price,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MaybePendingBlockWithTxHashes {
    Pending(PendingBlockWithTxHashes),
    Block(BlockWithTxHashes),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockWithReceipts(starknet::core::types::BlockWithReceipts);

impl BlockWithReceipts {
    pub fn new(
        header: Header,
        finality_status: FinalityStatus,
        receipts: impl Iterator<Item = (TxWithHash, Receipt)>,
    ) -> Self {
        let l1_gas_price = ResourcePrice {
            price_in_wei: header.l1_gas_prices.eth.into(),
            price_in_fri: header.l1_gas_prices.strk.into(),
        };

        let l1_data_gas_price = ResourcePrice {
            price_in_wei: header.l1_data_gas_prices.eth.into(),
            price_in_fri: header.l1_data_gas_prices.strk.into(),
        };

        let transactions = receipts
            .map(|(tx_with_hash, receipt)| {
                let receipt = TxReceipt::new(tx_with_hash.hash, finality_status, receipt).0;
                let transaction = Tx::from(tx_with_hash).0;
                TransactionWithReceipt { transaction, receipt }
            })
            .collect();

        Self(starknet::core::types::BlockWithReceipts {
            status: match finality_status {
                FinalityStatus::AcceptedOnL1 => BlockStatus::AcceptedOnL1,
                FinalityStatus::AcceptedOnL2 => BlockStatus::AcceptedOnL2,
            },
            block_hash: header.parent_hash,
            parent_hash: header.parent_hash,
            block_number: header.number,
            new_root: header.state_root,
            timestamp: header.timestamp,
            sequencer_address: header.sequencer_address.into(),
            l1_gas_price,
            l1_data_gas_price,
            l1_da_mode: L1DataAvailabilityMode::Calldata,
            starknet_version: header.protocol_version.to_string(),
            transactions,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PendingBlockWithReceipts(starknet::core::types::PendingBlockWithReceipts);

impl PendingBlockWithReceipts {
    pub fn new(
        header: PartialHeader,
        receipts: impl Iterator<Item = (TxWithHash, Receipt)>,
    ) -> Self {
        let l1_gas_price = ResourcePrice {
            price_in_wei: header.l1_gas_prices.eth.into(),
            price_in_fri: header.l1_gas_prices.strk.into(),
        };

        let l1_data_gas_price = ResourcePrice {
            price_in_wei: header.l1_data_gas_prices.eth.into(),
            price_in_fri: header.l1_data_gas_prices.strk.into(),
        };

        let transactions = receipts
            .map(|(tx_with_hash, receipt)| {
                let receipt =
                    TxReceipt::new(tx_with_hash.hash, FinalityStatus::AcceptedOnL2, receipt).0;
                let transaction = Tx::from(tx_with_hash).0;
                TransactionWithReceipt { transaction, receipt }
            })
            .collect();

        Self(starknet::core::types::PendingBlockWithReceipts {
            transactions,
            l1_gas_price,
            timestamp: header.timestamp,
            sequencer_address: header.sequencer_address.into(),
            parent_hash: header.parent_hash,
            l1_da_mode: match header.l1_da_mode {
                katana_primitives::da::L1DataAvailabilityMode::Blob => L1DataAvailabilityMode::Blob,
                katana_primitives::da::L1DataAvailabilityMode::Calldata => {
                    L1DataAvailabilityMode::Calldata
                }
            },
            l1_data_gas_price,
            starknet_version: header.protocol_version.to_string(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MaybePendingBlockWithReceipts {
    Pending(PendingBlockWithReceipts),
    Block(BlockWithReceipts),
}

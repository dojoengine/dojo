use std::collections::HashMap;

use starknet::core::types::StateUpdate;
use starknet_api::block::{
    Block, BlockBody, BlockHash, BlockHeader, BlockNumber, BlockStatus, BlockTimestamp, GasPrice,
};
use starknet_api::core::{ContractAddress, GlobalRoot};
use starknet_api::hash::{pedersen_hash_array, StarkFelt};
use starknet_api::stark_felt;
use starknet_api::transaction::{Transaction, TransactionOutput};

use crate::state::MemDb;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StarknetBlock {
    pub inner: Block,
    pub status: Option<BlockStatus>,
}

impl StarknetBlock {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        block_hash: BlockHash,
        parent_hash: BlockHash,
        block_number: BlockNumber,
        gas_price: GasPrice,
        state_root: GlobalRoot,
        sequencer: ContractAddress,
        timestamp: BlockTimestamp,
        transactions: Vec<Transaction>,
        transaction_outputs: Vec<TransactionOutput>,
        status: Option<BlockStatus>,
    ) -> Self {
        Self {
            inner: Block {
                header: BlockHeader {
                    block_hash,
                    parent_hash,
                    block_number,
                    gas_price,
                    state_root,
                    sequencer,
                    timestamp,
                },
                body: BlockBody { transactions, transaction_outputs },
            },
            status,
        }
    }

    pub fn header(&self) -> &BlockHeader {
        &self.inner.header
    }

    pub fn body(&self) -> &BlockBody {
        &self.inner.body
    }

    pub fn insert_transaction(&mut self, transaction: Transaction) {
        self.inner.body.transactions.push(transaction);
    }

    pub fn insert_transaction_output(&mut self, output: TransactionOutput) {
        self.inner.body.transaction_outputs.push(output);
    }

    pub fn transactions(&self) -> &[Transaction] {
        &self.inner.body.transactions
    }

    pub fn transaction_by_index(&self, transaction_id: usize) -> Option<Transaction> {
        self.inner.body.transactions.get(transaction_id).cloned()
    }

    pub fn block_hash(&self) -> BlockHash {
        self.inner.header.block_hash
    }

    pub fn block_number(&self) -> BlockNumber {
        self.inner.header.block_number
    }

    pub fn parent_hash(&self) -> BlockHash {
        self.inner.header.parent_hash
    }

    pub fn compute_block_hash(&self) -> BlockHash {
        BlockHash(pedersen_hash_array(&[
            stark_felt!(self.inner.header.block_number.0), // block number
            self.inner.header.state_root.0,                // global_state_root
            *self.inner.header.sequencer.0.key(),          // sequencer_address
            stark_felt!(self.inner.header.timestamp.0),    // block_timestamp
            stark_felt!(self.inner.body.transactions.len() as u64), // transaction_count
            stark_felt!(0_u8),                             // transaction_commitment
            stark_felt!(0_u8),                             // event_count
            stark_felt!(0_u8),                             // event_commitment
            stark_felt!(0_u8),                             // protocol_version
            stark_felt!(0_u8),                             // extra_data
            stark_felt!(self.parent_hash().0),             // parent_block_hash
        ]))
    }
}

// TODO: add state archive
#[derive(Debug, Default)]
pub struct StarknetBlocks {
    pub hash_to_num: HashMap<BlockHash, BlockNumber>,
    pub num_to_block: HashMap<BlockNumber, StarknetBlock>,
    pub pending_block: Option<StarknetBlock>,
    pub state_archive: HashMap<BlockNumber, MemDb>,
    pub num_to_state_update: HashMap<BlockNumber, StateUpdate>,
}

impl StarknetBlocks {
    pub fn insert(&mut self, block: StarknetBlock) {
        let block_number = block.block_number();
        self.hash_to_num.insert(block.block_hash(), block_number);
        self.num_to_block.insert(block_number, block);
    }

    pub fn current_block_number(&self) -> Option<BlockNumber> {
        let block_len = self.total_blocks();
        if block_len == 0 {
            None
        } else {
            Some(BlockNumber(block_len as u64 - 1))
        }
    }

    pub fn latest(&self) -> Option<StarknetBlock> {
        BlockNumber(self.num_to_block.len() as u64)
            .prev()
            .and_then(|num| self.num_to_block.get(&num).cloned())
    }

    pub fn by_hash(&self, block_hash: BlockHash) -> Option<StarknetBlock> {
        self.hash_to_num.get(&block_hash).and_then(|block_number| self.by_number(*block_number))
    }

    pub fn by_number(&self, block_number: BlockNumber) -> Option<StarknetBlock> {
        self.num_to_block.get(&block_number).cloned()
    }

    pub fn transaction_by_block_num_and_index(
        &self,
        number: BlockNumber,
        index: usize,
    ) -> Option<Transaction> {
        self.num_to_block.get(&number).and_then(|block| block.transaction_by_index(index))
    }

    pub fn total_blocks(&self) -> usize {
        self.num_to_block.len()
    }

    pub fn get_state_update(&self, block_number: BlockNumber) -> Option<StateUpdate> {
        self.num_to_state_update.get(&block_number).cloned()
    }

    pub fn get_state(&self, block_number: &BlockNumber) -> Option<&MemDb> {
        self.state_archive.get(block_number)
    }

    pub fn store_state(&mut self, block_number: BlockNumber, state: MemDb) {
        self.state_archive.insert(block_number, state);
    }
}

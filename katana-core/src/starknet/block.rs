use std::collections::HashMap;

use starknet_api::{
    block::{Block, BlockBody, BlockHash, BlockHeader, BlockNumber, BlockTimestamp, GasPrice},
    core::{ContractAddress, GlobalRoot},
    hash::{pedersen_hash_array, StarkFelt},
    stark_felt,
    state::StateUpdate,
    transaction::{Transaction, TransactionOutput},
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StarknetBlock(pub Block);

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
    ) -> Self {
        Self(Block {
            header: BlockHeader {
                block_hash,
                parent_hash,
                block_number,
                gas_price,
                state_root,
                sequencer,
                timestamp,
            },
            body: BlockBody {
                transactions,
                transaction_outputs,
            },
        })
    }

    pub fn insert_transaction(&mut self, transaction: Transaction) {
        self.0.body.transactions.push(transaction);
    }

    pub fn transactions(&self) -> &[Transaction] {
        &self.0.body.transactions
    }

    pub fn transaction_by_index(&self, transaction_id: usize) -> Option<Transaction> {
        self.0.body.transactions.get(transaction_id).cloned()
    }

    pub fn block_hash(&self) -> BlockHash {
        self.0.header.block_hash
    }

    pub fn block_number(&self) -> BlockNumber {
        self.0.header.block_number
    }

    pub fn parent_hash(&self) -> BlockHash {
        self.0.header.parent_hash
    }

    pub fn compute_block_hash(&self) -> BlockHash {
        BlockHash(pedersen_hash_array(&[
            stark_felt!(self.0.header.block_number.0), // block number
            stark_felt!(0),                            // global_state_root
            self.0.header.state_root.0,                // sequencer_address
            *self.0.header.sequencer.0.key(),          // block_timestamp
            stark_felt!(self.0.header.timestamp.0),    // transaction_count
            stark_felt!(self.0.body.transactions.len() as u64), // transaction_commitment
            stark_felt!(0),                            // event_count
            stark_felt!(0),                            // event_commitment
            stark_felt!(0),                            // protocol_version
            stark_felt!(0),                            // extra_data
            stark_felt!(self.parent_hash().0),         // parent_block_hash
        ]))
    }
}

// TODO: add state archive
#[derive(Debug, Default)]
pub struct StarknetBlocks {
    pub current_height: BlockNumber,
    pub hash_to_num: HashMap<BlockHash, BlockNumber>,
    pub num_to_block: HashMap<BlockNumber, StarknetBlock>,
    pub pending_block: Option<StarknetBlock>,
    pub num_to_state_update: HashMap<BlockNumber, StateUpdate>,
}

impl StarknetBlocks {
    pub fn append_block(&mut self, block: StarknetBlock) {
        let block_number = block.block_number();
        self.hash_to_num.insert(block.block_hash(), block_number);
        self.num_to_block.insert(block_number, block);
    }

    pub fn lastest(&self) -> Option<StarknetBlock> {
        self.current_height
            .prev()
            .and_then(|block_number| self.num_to_block.get(&block_number).cloned())
    }

    pub fn by_hash(&self, block_hash: BlockHash) -> Option<StarknetBlock> {
        self.hash_to_num
            .get(&block_hash)
            .and_then(|block_number| self.by_number(*block_number))
    }

    pub fn by_number(&self, block_number: BlockNumber) -> Option<StarknetBlock> {
        self.num_to_block.get(&block_number).cloned()
    }

    pub fn transaction_by_block_num_and_index(
        &self,
        number: BlockNumber,
        index: usize,
    ) -> Option<Transaction> {
        self.num_to_block
            .get(&number)
            .and_then(|block| block.transaction_by_index(index))
    }
}

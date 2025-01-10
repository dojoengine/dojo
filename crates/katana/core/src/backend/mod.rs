use std::sync::Arc;

use gas_oracle::L1GasOracle;
use katana_executor::{ExecutionOutput, ExecutionResult, ExecutorFactory};
use katana_primitives::block::{
    FinalityStatus, Header, PartialHeader, SealedBlock, SealedBlockWithStatus,
};
use katana_primitives::chain_spec::ChainSpec;
use katana_primitives::da::L1DataAvailabilityMode;
use katana_primitives::env::BlockEnv;
use katana_primitives::receipt::{Event, ReceiptWithTxHash};
use katana_primitives::state::{compute_state_diff_hash, StateUpdates};
use katana_primitives::transaction::{TxHash, TxWithHash};
use katana_primitives::{address, ContractAddress, Felt};
use katana_provider::traits::block::{BlockHashProvider, BlockWriter};
use katana_provider::traits::trie::TrieWriter;
use katana_trie::compute_merkle_root;
use parking_lot::RwLock;
use starknet::macros::short_string;
use starknet_types_core::hash::{self, StarkHash};
use tracing::info;

pub mod contract;
pub mod gas_oracle;
pub mod storage;

use self::storage::Blockchain;
use crate::env::BlockContextGenerator;
use crate::service::block_producer::{BlockProductionError, MinedBlockOutcome};
use crate::utils::get_current_timestamp;

pub(crate) const LOG_TARGET: &str = "katana::core::backend";

#[derive(Debug)]
pub struct Backend<EF: ExecutorFactory> {
    pub chain_spec: Arc<ChainSpec>,
    /// stores all block related data in memory
    pub blockchain: Blockchain,
    /// The block context generator.
    pub block_context_generator: RwLock<BlockContextGenerator>,

    pub executor_factory: Arc<EF>,

    pub gas_oracle: L1GasOracle,
}

impl<EF: ExecutorFactory> Backend<EF> {
    // TODO: add test for this function
    pub fn do_mine_block(
        &self,
        block_env: &BlockEnv,
        mut execution_output: ExecutionOutput,
    ) -> Result<MinedBlockOutcome, BlockProductionError> {
        // we optimistically allocate the maximum amount possible
        let mut txs = Vec::with_capacity(execution_output.transactions.len());
        let mut traces = Vec::with_capacity(execution_output.transactions.len());
        let mut receipts = Vec::with_capacity(execution_output.transactions.len());

        // only include successful transactions in the block
        for (tx, res) in execution_output.transactions {
            if let ExecutionResult::Success { receipt, trace, .. } = res {
                receipts.push(ReceiptWithTxHash::new(tx.hash, receipt));
                traces.push(trace);
                txs.push(tx);
            }
        }

        let tx_count = txs.len() as u32;
        let tx_hashes = txs.iter().map(|tx| tx.hash).collect::<Vec<TxHash>>();

        // Update special contract address 0x1
        self.update_block_hash_registry_contract(
            &mut execution_output.states.state_updates,
            block_env.number,
        )?;

        // create a new block and compute its commitment
        let block = self.commit_block(
            block_env.clone(),
            execution_output.states.state_updates.clone(),
            txs,
            &receipts,
        )?;

        let block = SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL2 };
        let block_number = block.block.header.number;

        // TODO: maybe should change the arguments for insert_block_with_states_and_receipts to
        // accept ReceiptWithTxHash instead to avoid this conversion.
        let receipts = receipts.into_iter().map(|r| r.receipt).collect::<Vec<_>>();
        self.blockchain.provider().insert_block_with_states_and_receipts(
            block,
            execution_output.states,
            receipts,
            traces,
        )?;

        info!(target: LOG_TARGET, %block_number, %tx_count, "Block mined.");
        Ok(MinedBlockOutcome { block_number, txs: tx_hashes, stats: execution_output.stats })
    }

    // TODO: create a dedicated class for this contract.
    // https://docs.starknet.io/architecture-and-concepts/network-architecture/starknet-state/#address_0x1
    fn update_block_hash_registry_contract(
        &self,
        state_updates: &mut StateUpdates,
        block_number: u64,
    ) -> Result<(), BlockProductionError> {
        const STORED_BLOCK_HASH_BUFFER: u64 = 10;

        if block_number >= STORED_BLOCK_HASH_BUFFER {
            let block_number = block_number - STORED_BLOCK_HASH_BUFFER;
            let block_hash = self
                .blockchain
                .provider()
                .block_hash_by_num(block_number)?
                .expect("qed; missing block hash");

            let storages = state_updates.storage_updates.entry(address!("0x1")).or_default();
            storages.insert(block_number.into(), block_hash);
        }

        Ok(())
    }

    pub fn update_block_env(&self, block_env: &mut BlockEnv) {
        let mut context_gen = self.block_context_generator.write();
        let current_timestamp_secs = get_current_timestamp().as_secs() as i64;

        let timestamp = if context_gen.next_block_start_time == 0 {
            (current_timestamp_secs + context_gen.block_timestamp_offset) as u64
        } else {
            let timestamp = context_gen.next_block_start_time;
            context_gen.block_timestamp_offset = timestamp as i64 - current_timestamp_secs;
            context_gen.next_block_start_time = 0;
            timestamp
        };

        block_env.number += 1;
        block_env.timestamp = timestamp;

        // update the gas prices
        self.update_block_gas_prices(block_env);
    }

    /// Updates the gas prices in the block environment.
    pub fn update_block_gas_prices(&self, block_env: &mut BlockEnv) {
        block_env.l1_gas_prices = self.gas_oracle.current_gas_prices();
        block_env.l1_data_gas_prices = self.gas_oracle.current_data_gas_prices();
    }

    pub fn mine_empty_block(
        &self,
        block_env: &BlockEnv,
    ) -> Result<MinedBlockOutcome, BlockProductionError> {
        self.do_mine_block(block_env, Default::default())
    }

    fn commit_block(
        &self,
        block_env: BlockEnv,
        state_updates: StateUpdates,
        transactions: Vec<TxWithHash>,
        receipts: &[ReceiptWithTxHash],
    ) -> Result<SealedBlock, BlockProductionError> {
        let parent_hash = self.blockchain.provider().latest_hash()?;
        let partial_header = PartialHeader {
            parent_hash,
            number: block_env.number,
            timestamp: block_env.timestamp,
            protocol_version: self.chain_spec.version.clone(),
            sequencer_address: block_env.sequencer_address,
            l1_gas_prices: block_env.l1_gas_prices,
            l1_data_gas_prices: block_env.l1_data_gas_prices,
            l1_da_mode: L1DataAvailabilityMode::Calldata,
        };

        let block = UncommittedBlock::new(
            partial_header,
            transactions,
            receipts,
            &state_updates,
            &self.blockchain.provider(),
        )
        .commit();
        Ok(block)
    }
}

#[derive(Debug, Clone)]
pub struct UncommittedBlock<'a, P: TrieWriter> {
    header: PartialHeader,
    transactions: Vec<TxWithHash>,
    receipts: &'a [ReceiptWithTxHash],
    state_updates: &'a StateUpdates,
    provider: P,
}

impl<'a, P: TrieWriter> UncommittedBlock<'a, P> {
    pub fn new(
        header: PartialHeader,
        transactions: Vec<TxWithHash>,
        receipts: &'a [ReceiptWithTxHash],
        state_updates: &'a StateUpdates,
        trie_provider: P,
    ) -> Self {
        Self { header, transactions, receipts, state_updates, provider: trie_provider }
    }

    pub fn commit(self) -> SealedBlock {
        // get the hash of the latest committed block
        let parent_hash = self.header.parent_hash;
        let events_count = self.receipts.iter().map(|r| r.events().len() as u32).sum::<u32>();
        let transaction_count = self.transactions.len() as u32;
        let state_diff_length = self.state_updates.len() as u32;

        let state_root = self.compute_new_state_root();
        let transactions_commitment = self.compute_transaction_commitment();
        let events_commitment = self.compute_event_commitment();
        let receipts_commitment = self.compute_receipt_commitment();
        let state_diff_commitment = self.compute_state_diff_commitment();

        let header = Header {
            state_root,
            parent_hash,
            events_count,
            state_diff_length,
            transaction_count,
            events_commitment,
            receipts_commitment,
            state_diff_commitment,
            transactions_commitment,
            number: self.header.number,
            timestamp: self.header.timestamp,
            l1_da_mode: self.header.l1_da_mode,
            l1_gas_prices: self.header.l1_gas_prices,
            l1_data_gas_prices: self.header.l1_data_gas_prices,
            sequencer_address: self.header.sequencer_address,
            protocol_version: self.header.protocol_version,
        };

        let hash = header.compute_hash();

        SealedBlock { hash, header, body: self.transactions }
    }

    fn compute_transaction_commitment(&self) -> Felt {
        let tx_hashes = self.transactions.iter().map(|t| t.hash).collect::<Vec<TxHash>>();
        compute_merkle_root::<hash::Poseidon>(&tx_hashes).unwrap()
    }

    fn compute_receipt_commitment(&self) -> Felt {
        let receipt_hashes = self.receipts.iter().map(|r| r.compute_hash()).collect::<Vec<Felt>>();
        compute_merkle_root::<hash::Poseidon>(&receipt_hashes).unwrap()
    }

    fn compute_state_diff_commitment(&self) -> Felt {
        compute_state_diff_hash(self.state_updates.clone())
    }

    fn compute_event_commitment(&self) -> Felt {
        // h(emitter_address, tx_hash, h(keys), h(data))
        fn event_hash(tx: TxHash, event: &Event) -> Felt {
            let keys_hash = hash::Poseidon::hash_array(&event.keys);
            let data_hash = hash::Poseidon::hash_array(&event.data);
            hash::Poseidon::hash_array(&[tx, event.from_address.into(), keys_hash, data_hash])
        }

        // the iterator will yield all events from all the receipts, each one paired with the
        // transaction hash that emitted it: (tx hash, event).
        let events = self.receipts.iter().flat_map(|r| r.events().iter().map(|e| (r.tx_hash, e)));

        let mut hashes = Vec::new();
        for (tx, event) in events {
            let event_hash = event_hash(tx, event);
            hashes.push(event_hash);
        }

        // compute events commitment
        compute_merkle_root::<hash::Poseidon>(&hashes).unwrap()
    }

    // state_commitment = hPos("STARKNET_STATE_V0", contract_trie_root, class_trie_root)
    fn compute_new_state_root(&self) -> Felt {
        let class_trie_root = self
            .provider
            .trie_insert_declared_classes(self.header.number, &self.state_updates.declared_classes)
            .expect("failed to update class trie");

        let contract_trie_root = self
            .provider
            .trie_insert_contract_updates(self.header.number, self.state_updates)
            .expect("failed to update contract trie");

        hash::Poseidon::hash_array(&[
            short_string!("STARKNET_STATE_V0"),
            contract_trie_root,
            class_trie_root,
        ])
    }
}

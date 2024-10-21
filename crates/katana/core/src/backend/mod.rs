use std::sync::Arc;

use katana_executor::{ExecutionOutput, ExecutionResult, ExecutorFactory};
use katana_primitives::block::{
    Block, FinalityStatus, GasPrices, Header, PartialHeader, SealedBlock, SealedBlockWithStatus,
};
use katana_primitives::chain_spec::ChainSpec;
use katana_primitives::da::L1DataAvailabilityMode;
use katana_primitives::env::BlockEnv;
use katana_primitives::receipt::{Event, Receipt, ReceiptWithTxHash};
use katana_primitives::state::{compute_state_diff_hash, StateUpdates};
use katana_primitives::transaction::{TxHash, TxWithHash};
use katana_primitives::{ContractAddress, Felt};
use katana_provider::traits::block::{BlockHashProvider, BlockWriter};
use katana_trie::trie::compute_merkle_root;
use parking_lot::RwLock;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::macros::short_string;
use starknet_types_core::hash::{self, StarkHash};
use tracing::info;

pub mod contract;
pub mod storage;

use self::storage::Blockchain;
use crate::env::BlockContextGenerator;
use crate::service::block_producer::{BlockProductionError, MinedBlockOutcome};
use crate::utils::get_current_timestamp;

pub(crate) const LOG_TARGET: &str = "katana::core::backend";

#[derive(Debug)]
pub struct Backend<EF: ExecutorFactory> {
    pub chain_spec: ChainSpec,
    /// stores all block related data in memory
    pub blockchain: Blockchain,
    /// The block context generator.
    pub block_context_generator: RwLock<BlockContextGenerator>,

    pub executor_factory: Arc<EF>,
}

impl<EF: ExecutorFactory> Backend<EF> {
    // TODO: add test for this function
    pub fn do_mine_block(
        &self,
        block_env: &BlockEnv,
        execution_output: ExecutionOutput,
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

        // create a new block and compute its commitment
        let block = self.commit_block(
            block_env,
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
    }

    pub fn mine_empty_block(
        &self,
        block_env: &BlockEnv,
    ) -> Result<MinedBlockOutcome, BlockProductionError> {
        self.do_mine_block(block_env, Default::default())
    }

    fn commit_block(
        &self,
        block_env: &BlockEnv,
        state_updates: StateUpdates,
        transactions: Vec<TxWithHash>,
        receipts: &[ReceiptWithTxHash],
    ) -> Result<SealedBlock, BlockProductionError> {
        // let block = UncommittedBlock::new(header, transactions, &receipts, &state_updates);
        // let committed = block.commit();
        // let sealed = Block { header, body: transactions }.seal();
        // Ok(sealed)

        todo!()
    }
}

#[derive(Debug, Clone)]
pub struct UncommittedBlock<'a> {
    header: PartialHeader,
    transactions: Vec<TxWithHash>,
    receipts: &'a [ReceiptWithTxHash],
    state_updates: &'a StateUpdates,
}

impl<'a> UncommittedBlock<'a> {
    pub fn new(
        header: PartialHeader,
        transactions: Vec<TxWithHash>,
        receipts: &'a [ReceiptWithTxHash],
        states: &'a StateUpdates,
    ) -> Self {
        Self { header, transactions, receipts, state_updates: states }
    }

    pub fn commit(self) -> SealedBlock {
        // get the hash of the latest committed block
        let parent_hash = self.header.parent_hash;
        let events_count = self.receipts.iter().map(|r| r.events().len() as u32).sum::<u32>();
        let transaction_count = self.transactions.len() as u32;
        let state_diff_length = self.state_updates.len() as u32;

        let l1_gas_prices =
            GasPrices { eth: self.header.l1_gas_prices.eth, strk: self.header.l1_gas_prices.strk };
        let l1_data_gas_prices = GasPrices {
            eth: self.header.l1_data_gas_prices.eth,
            strk: self.header.l1_data_gas_prices.strk,
        };

        // Computes the block hash.
        //
        // A block hash is defined as the Poseidon hash of the header’s fields, as follows:
        //
        // h(𝐵) = h(
        //     "STARKNET_BLOCK_HASH0",
        //     block_number,
        //     global_state_root,
        //     sequencer_address,
        //     block_timestamp,
        //     transaction_count || event_count || state_diff_length || l1_da_mode,
        //     state_diff_commitment,
        //     transactions_commitment
        //     events_commitment,
        //     receipts_commitment
        //     l1_gas_price_in_wei,
        //     l1_gas_price_in_fri,
        //     l1_data_gas_price_in_wei,
        //     l1_data_gas_price_in_fri
        //     protocol_version,
        //     0,
        //     parent_block_hash
        // )
        //
        // Based on StarkWare's [Sequencer implementation].
        //
        // [Sequencer implementation]: https://github.com/starkware-libs/sequencer/blob/bb361ec67396660d5468fd088171913e11482708/crates/starknet_api/src/block_hash/block_hash_calculator.rs#L62-L93
        let starknet_version = self.header.protocol_version.to_string();
        let starknet_version = cairo_short_string_to_felt(&starknet_version).unwrap();

        let concat = Self::concat_counts(
            transaction_count,
            events_count,
            state_diff_length,
            self.header.l1_da_mode,
        );

        let block_hash = hash::Poseidon::hash_array(&[
            short_string!("STARKNET_BLOCK_HASH0"),
            self.header.number.into(),
            Felt::ZERO, // self.header.state_root,
            self.header.sequencer_address.into(),
            self.header.timestamp.into(),
            concat,
            self.state_diff_commitment,
            self.transactions_commitment,
            self.events_commitment,
            self.receipts_commitment,
            self.header.l1_gas_prices.eth.into(),
            self.header.l1_gas_prices.strk.into(),
            self.header.l1_data_gas_prices.eth.into(),
            self.header.l1_data_gas_prices.strk.into(),
            starknet_version,
            Felt::ZERO,
            self.header.parent_hash,
        ]);

        let header = Header {
            parent_hash,
            events_count,
            state_root,
            l1_gas_prices,
            l1_data_gas_prices,
            transaction_count,
            events_commitment,
            receipts_commitment,
            state_diff_commitment,
            transactions_commitment,
            number: block_env.number,
            timestamp: block_env.timestamp,
            l1_da_mode: L1DataAvailabilityMode::Calldata,
            sequencer_address: block_env.sequencer_address,
            protocol_version: self.chain_spec.version.clone(),
        };

        SealedBlock { hash: block_hash, header, body: self.transactions }
    }

    fn compute_transaction_commitment(&self) -> Felt {
        let tx_hashes = self.transactions.iter().map(|t| t.hash).collect::<Vec<TxHash>>();
        let commitment = compute_merkle_root::<hash::Poseidon>(&tx_hashes).unwrap();
        commitment
    }

    fn compute_receipt_commitment(&self) -> Felt {
        let receipt_hashes = self.receipts.iter().map(|r| r.compute_hash()).collect::<Vec<Felt>>();
        let commitment = compute_merkle_root::<hash::Poseidon>(&receipt_hashes).unwrap();
        commitment
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
        let events =
            self.receipts.iter().map(|r| r.events().iter().map(|e| (r.tx_hash, e))).flatten();

        let mut hashes = Vec::new();
        for (tx, event) in events {
            let event_hash = event_hash(tx, event);
            hashes.push(event_hash);
        }

        // compute events commitment
        let commitment = compute_merkle_root::<hash::Poseidon>(&hashes).unwrap();
        commitment
    }

    // Concantenate the transaction_count, event_count and state_diff_length, and l1_da_mode into a
    // single felt.
    //
    // A single felt:
    //
    // +-------------------+----------------+----------------------+--------------+------------+
    // | transaction_count | event_count    | state_diff_length    | L1 DA mode   | padding    |
    // | (64 bits)         | (64 bits)      | (64 bits)            | (1 bit)      | (63 bit)   |
    // +-------------------+----------------+----------------------+--------------+------------+
    //
    // where, L1 DA mode is 0 for calldata, and 1 for blob.
    //
    // Taken from https://github.com/starkware-libs/sequencer/blob/bb361ec67396660d5468fd088171913e11482708/crates/starknet_api/src/block_hash/block_hash_calculator.rs#L135-L164
    fn concat_counts(
        transaction_count: u32,
        event_count: u32,
        state_diff_length: u32,
        l1_data_availability_mode: L1DataAvailabilityMode,
    ) -> Felt {
        fn to_64_bits(num: u32) -> [u8; 8] {
            (num as u64).to_be_bytes()
        }

        let l1_data_availability_byte: u8 = match l1_data_availability_mode {
            L1DataAvailabilityMode::Calldata => 0,
            L1DataAvailabilityMode::Blob => 0b10000000,
        };

        let concat_bytes = [
            to_64_bits(transaction_count).as_slice(),
            to_64_bits(event_count).as_slice(),
            to_64_bits(state_diff_length).as_slice(),
            &[l1_data_availability_byte],
            &[0_u8; 7], // zero padding
        ]
        .concat();

        Felt::from_bytes_be_slice(concat_bytes.as_slice())
    }
}

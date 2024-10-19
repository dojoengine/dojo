use std::sync::Arc;

use katana_db::tables::Receipts;
use katana_executor::{ExecutionOutput, ExecutionResult, ExecutorFactory};
use katana_primitives::block::{
    Block, FinalityStatus, GasPrices, Header, PartialHeader, SealedBlock, SealedBlockWithStatus,
};
use katana_primitives::chain_spec::ChainSpec;
use katana_primitives::da::L1DataAvailabilityMode;
use katana_primitives::env::BlockEnv;
use katana_primitives::receipt::Receipt;
use katana_primitives::transaction::{TxHash, TxWithHash};
use katana_primitives::Felt;
use katana_provider::traits::block::{BlockHashProvider, BlockWriter};
use parking_lot::RwLock;
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
                txs.push(tx);
                traces.push(trace);
                receipts.push(receipt);
            }
        }

        let tx_count = txs.len() as u32;
        let tx_hashes = txs.iter().map(|tx| tx.hash).collect::<Vec<TxHash>>();

        // create a new block and compute its commitment
        let block = self.commit_block(block_env, txs, &receipts)?;
        let block = SealedBlockWithStatus { block, status: FinalityStatus::AcceptedOnL2 };
        let block_number = block.block.header.header.number;

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
        transactions: Vec<TxWithHash>,
        receipts: &[Receipt],
    ) -> Result<SealedBlock, BlockProductionError> {
        let parent_hash = self.blockchain.provider().latest_hash()?;
        let events_count = receipts.iter().map(|r| r.events().len() as u32).sum::<u32>();
        let transaction_count = transactions.len() as u32;

        let l1_gas_prices =
            GasPrices { eth: block_env.l1_gas_prices.eth, strk: block_env.l1_gas_prices.strk };
        let l1_data_gas_prices = GasPrices {
            eth: block_env.l1_data_gas_prices.eth,
            strk: block_env.l1_data_gas_prices.strk,
        };

        let header = Header {
            parent_hash,
            events_count,
            l1_gas_prices,
            transaction_count,
            l1_data_gas_prices,
            state_root: Felt::ZERO,
            number: block_env.number,
            events_commitment: Felt::ZERO,
            timestamp: block_env.timestamp,
            receipts_commitment: Felt::ZERO,
            state_diff_commitment: Felt::ZERO,
            transactions_commitment: Felt::ZERO,
            l1_da_mode: L1DataAvailabilityMode::Calldata,
            sequencer_address: block_env.sequencer_address,
            protocol_version: self.chain_spec.version.clone(),
        };

        Ok(Block { header, body: transactions }.seal())
    }
}

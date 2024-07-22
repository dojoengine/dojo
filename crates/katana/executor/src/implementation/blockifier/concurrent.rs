use std::sync::Arc;

use blockifier::blockifier::transaction_executor::{TransactionExecutor, TransactionExecutorResult};
use blockifier::state::cached_state::CachedState;
use blockifier::transaction::transactions::ExecutableTransaction;
use katana_primitives::block::{ExecutableBlock, PartialHeader};
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, TxWithHash};
use katana_provider::traits::state::StateProvider;

use super::state::CachedState as KatanaCachedState;
use super::utils;
use super::StarknetVMProcessor;
use crate::{BlockExecutor, ExecutionOutput, ExecutionResult, ExecutorExt, ExecutorResult, StateProviderDb};


pub struct ConcurrentBlockExecutor<'a> {
    inner: StarknetVMProcessor<'a>,
    tx_executor: TransactionExecutor<CachedState<StateProviderDb<'a>>>,
}

impl<'a> ConcurrentBlockExecutor<'a> {
    pub fn new(processor: StarknetVMProcessor<'a>) -> Self {
        let block_context = processor.block_context.clone();
        let state = processor.state.clone();
        // let config = Default::default(); // TODO: Configure this properly

        // let tx_executor = TransactionExecutor::new(state, block_context, config);

     todo!()
    }
}

impl<'a> BlockExecutor<'a> for ConcurrentBlockExecutor<'a> {
    fn execute_block(&mut self, block: ExecutableBlock) -> ExecutorResult<()> {
        self.inner.fill_block_env_from_header(&block.header);
        self.execute_transactions(block.body)
    }

    fn execute_transactions(
        &mut self,
        transactions: Vec<ExecutableTxWithHash>,
    ) -> ExecutorResult<()> {
        let txs = transactions
            .into_iter()
            .map(|tx| utils::to_executor_tx(tx.clone()))
            .collect::<Vec<_>>();

        let results = self.tx_executor.execute_txs(&txs);


        Ok(())
    }

    fn take_execution_output(&mut self) -> ExecutorResult<ExecutionOutput> {
        let (state_diff, _, weights) = self.tx_executor.finalize().unwrap();
        let states = utils::state_update_from_cached_state(state_diff);
        let transactions = std::mem::take(&mut self.inner.transactions);
        let stats = std::mem::take(&mut self.inner.stats);
        
        // Update stats based on weights
        // TODO: Implement this properly
        
        Ok(ExecutionOutput { stats, states, transactions })
    }

    fn state(&self) -> Box<dyn StateProvider + 'a> {
        self.inner.state()
    }

    fn transactions(&self) -> &[(TxWithHash, ExecutionResult)] {
        self.inner.transactions()
    }

    fn block_env(&self) -> katana_primitives::env::BlockEnv {
        self.inner.block_env()
    }
}


impl<'a> ExecutorExt for ConcurrentBlockExecutor<'a> {
    fn call(&self, call: crate::EntryPointCall) -> Result<Vec<katana_primitives::FieldElement>, crate::ExecutionError> {
        unimplemented!()
    }

    fn estimate_fee(
            &self,
            transactions: Vec<ExecutableTxWithHash>,
            flags: crate::SimulationFlag,
        ) -> Vec<Result<katana_primitives::fee::TxFeeInfo, crate::ExecutionError>> {
        unimplemented!()
    }

    fn simulate(
            &self,
            transactions: Vec<ExecutableTxWithHash>,
            flags: crate::SimulationFlag,
        ) -> Vec<crate::ResultAndStates> {
        unimplemented!()
    }
}
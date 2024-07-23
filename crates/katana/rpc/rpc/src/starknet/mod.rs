//! Server implementation for the Starknet JSON-RPC API.

mod read;
mod trace;
mod write;

use std::sync::Arc;

use katana_core::sequencer::KatanaSequencer;
use katana_executor::ExecutorFactory;
use katana_primitives::block::BlockIdOrTag;
use katana_primitives::transaction::ExecutableTxWithHash;
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::FeeEstimate;
use katana_tasks::{BlockingTaskPool, TokioTaskSpawner};

#[allow(missing_debug_implementations)]
pub struct StarknetApi<EF: ExecutorFactory> {
    inner: Arc<Inner<EF>>,
}

impl<EF: ExecutorFactory> Clone for StarknetApi<EF> {
    fn clone(&self) -> Self {
        Self { inner: Arc::clone(&self.inner) }
    }
}

struct Inner<EF: ExecutorFactory> {
    sequencer: Arc<KatanaSequencer<EF>>,
    blocking_task_pool: BlockingTaskPool,
}

impl<EF: ExecutorFactory> StarknetApi<EF> {
    pub fn new(sequencer: Arc<KatanaSequencer<EF>>) -> Self {
        let blocking_task_pool =
            BlockingTaskPool::new().expect("failed to create blocking task pool");
        Self { inner: Arc::new(Inner { sequencer, blocking_task_pool }) }
    }

    async fn on_cpu_blocking_task<F, T>(&self, func: F) -> T
    where
        F: FnOnce(Self) -> T + Send + 'static,
        T: Send + 'static,
    {
        let this = self.clone();
        self.inner.blocking_task_pool.spawn(move || func(this)).await.unwrap()
    }

    async fn on_io_blocking_task<F, T>(&self, func: F) -> T
    where
        F: FnOnce(Self) -> T + Send + 'static,
        T: Send + 'static,
    {
        let this = self.clone();
        TokioTaskSpawner::new().unwrap().spawn_blocking(move || func(this)).await.unwrap()
    }

    fn estimate_fee_with(
        &self,
        transactions: Vec<ExecutableTxWithHash>,
        block_id: BlockIdOrTag,
        flags: katana_executor::SimulationFlag,
    ) -> Result<Vec<FeeEstimate>, StarknetApiError> {
        let sequencer = &self.inner.sequencer;
        // get the state and block env at the specified block for execution
        let state = sequencer.state(&block_id).map_err(StarknetApiError::from)?;
        let env = sequencer
            .block_env_at(block_id)
            .map_err(StarknetApiError::from)?
            .ok_or(StarknetApiError::BlockNotFound)?;

        // create the executor
        let executor = sequencer.backend.executor_factory.with_state_and_block_env(state, env);
        let results = executor.estimate_fee(transactions, flags);

        let mut estimates = Vec::with_capacity(results.len());
        for (i, res) in results.into_iter().enumerate() {
            match res {
                Ok(fee) => estimates.push(FeeEstimate {
                    gas_price: fee.gas_price.into(),
                    gas_consumed: fee.gas_consumed.into(),
                    overall_fee: fee.overall_fee.into(),
                    unit: fee.unit,
                    data_gas_price: Default::default(),
                    data_gas_consumed: Default::default(),
                }),

                Err(err) => {
                    return Err(StarknetApiError::TransactionExecutionError {
                        transaction_index: i,
                        execution_error: err.to_string(),
                    });
                }
            }
        }

        Ok(estimates)
    }
}

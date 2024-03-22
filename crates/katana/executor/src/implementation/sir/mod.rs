mod error;
mod output;
mod state;
pub mod utils;

use katana_primitives::block::{ExecutableBlock, PartialHeader};
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::fee::TxFeeInfo;
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, TxWithHash};
use katana_primitives::FieldElement;
use katana_provider::traits::state::StateProvider;
use sir::definitions::block_context::{self, BlockContext};
use sir::execution::TransactionExecutionInfo;
use sir::state::cached_state;
use sir::state::contract_class_cache::PermanentContractClassCache;
use std::sync::Arc;
use tracing::info;

use self::output::receipt_from_exec_info;
use self::state::CachedState;
use crate::abstraction::{BlockExecutor, ExecutorExt, ExecutorFactory};
use crate::abstraction::{ExecutionOutput, ExecutorResult, SimulationFlag, StateProviderDb};
use crate::{EntryPointCall, ExecutionError, ExecutionResult, ResultAndStates};

/// A factory for creating [StarknetVMProcessor] instances.
#[derive(Debug)]
pub struct NativeExecutorFactory {
    cfg: CfgEnv,
    flags: SimulationFlag,
}

impl NativeExecutorFactory {
    /// Create a new factory with the given configuration and simulation flags.
    pub fn new(cfg: CfgEnv, flags: SimulationFlag) -> Self {
        Self { cfg, flags }
    }
}

impl ExecutorFactory for NativeExecutorFactory {
    fn with_state<'a, P>(&self, state: P) -> Box<dyn BlockExecutor<'a> + 'a>
    where
        P: StateProvider + 'a,
    {
        self.with_state_and_block_env(state, BlockEnv::default())
    }

    fn with_state_and_block_env<'a, P>(
        &self,
        state: P,
        block_env: BlockEnv,
    ) -> Box<dyn BlockExecutor<'a> + 'a>
    where
        P: StateProvider + 'a,
    {
        let cfg_env = self.cfg.clone();
        let simulation_flags = self.flags.clone();
        Box::new(StarknetVMProcessor::new(Box::new(state), block_env, cfg_env, simulation_flags))
    }

    fn cfg(&self) -> &CfgEnv {
        &self.cfg
    }
}

pub struct StarknetVMProcessor<'a> {
    block_context: BlockContext,
    state: CachedState<StateProviderDb<'a>, PermanentContractClassCache>,
    transactions: Vec<(TxWithHash, ExecutionResult)>,
    simulation_flags: SimulationFlag,
}

impl<'a> StarknetVMProcessor<'a> {
    pub fn new(
        state: Box<dyn StateProvider + 'a>,
        block_env: BlockEnv,
        cfg_env: CfgEnv,
        simulation_flags: SimulationFlag,
    ) -> Self {
        let transactions = Vec::new();
        let block_context = utils::block_context_from_envs(&block_env, &cfg_env);
        let state =
            CachedState::new(StateProviderDb(state), PermanentContractClassCache::default());
        Self { block_context, state, transactions, simulation_flags }
    }

    fn fill_block_env_from_header(&mut self, header: &PartialHeader) {
        let number = header.number;
        let timestamp = header.timestamp;
        let sequencer_address = utils::to_sir_address(&header.sequencer_address);

        let gas_prices = block_context::GasPrices {
            eth_l1_gas_price: header.gas_prices.eth,
            strk_l1_gas_price: header.gas_prices.strk,
        };

        self.block_context.block_info_mut().block_number = number;
        self.block_context.block_info_mut().gas_price = gas_prices;
        self.block_context.block_info_mut().block_timestamp = timestamp;
        self.block_context.block_info_mut().sequencer_address = sequencer_address;
    }

    fn simulate_with<F, T>(
        &self,
        transactions: Vec<ExecutableTxWithHash>,
        flags: &SimulationFlag,
        mut op: F,
    ) -> Vec<T>
    where
        F: FnMut((TxWithHash, Result<(TransactionExecutionInfo, TxFeeInfo), ExecutionError>)) -> T,
    {
        let block_context = &self.block_context;

        let mut state = cached_state::CachedState::new(
            Arc::new(&self.state),
            Arc::new(PermanentContractClassCache::default()),
        );

        let mut results = Vec::with_capacity(transactions.len());
        for exec_tx in transactions {
            let tx = TxWithHash::from(&exec_tx);
            let res = utils::transact(exec_tx, &mut state, block_context, flags);

            results.push(op((tx, res)));
        }

        results
    }
}

impl<'a> BlockExecutor<'a> for StarknetVMProcessor<'a> {
    fn execute_transactions(
        &mut self,
        transactions: Vec<ExecutableTxWithHash>,
    ) -> ExecutorResult<()> {
        let block_context = &self.block_context;
        let flags = &self.simulation_flags;
        let mut state = self.state.0.write();

        for exec_tx in transactions {
            // Collect class artifacts if its a declare tx
            let class_decl_artifacts = if let ExecutableTx::Declare(tx) = exec_tx.as_ref() {
                let class_hash = tx.class_hash();
                Some((class_hash, tx.compiled_class.clone(), tx.sierra_class.clone()))
            } else {
                None
            };

            let tx = TxWithHash::from(&exec_tx);
            let res = match utils::transact(exec_tx, &mut state.inner, block_context, flags) {
                Ok((info, fee)) => {
                    // get the trace and receipt from the execution info
                    let trace = utils::to_exec_info(&info);
                    let receipt = receipt_from_exec_info(&tx, &trace);
                    ExecutionResult::new_success(receipt, trace, fee)
                }
                Err(e) => {
                    info!(target: "executor", "transaction execution failed: {e}");
                    ExecutionResult::new_failed(e)
                }
            };

            // if the tx succeed, inserts the class artifacts into the contract class cache
            if res.is_success() {
                if let Some((class_hash, compiled, sierra)) = class_decl_artifacts {
                    state.declared_classes.insert(class_hash, (compiled, sierra));
                }
            }

            self.transactions.push((tx, res));
        }

        Ok(())
    }

    fn execute_block(&mut self, block: ExecutableBlock) -> ExecutorResult<()> {
        self.fill_block_env_from_header(&block.header);
        self.execute_transactions(block.body)?;
        Ok(())
    }

    fn take_execution_output(&mut self) -> ExecutorResult<ExecutionOutput> {
        let states = utils::state_update_from_cached_state(&self.state);
        let transactions = std::mem::take(&mut self.transactions);
        Ok(ExecutionOutput { states, transactions })
    }

    fn state(&self) -> Box<dyn StateProvider + 'a> {
        Box::new(self.state.clone())
    }

    fn transactions(&self) -> &[(TxWithHash, ExecutionResult)] {
        &self.transactions
    }

    fn block_env(&self) -> BlockEnv {
        BlockEnv {
            number: self.block_context.block_info().block_number,
            timestamp: self.block_context.block_info().block_timestamp,
            sequencer_address: utils::to_address(
                &self.block_context.block_info().sequencer_address,
            ),
            l1_gas_prices: katana_primitives::block::GasPrices {
                eth: self.block_context.block_info().gas_price.eth_l1_gas_price,
                strk: self.block_context.block_info().gas_price.strk_l1_gas_price,
            },
        }
    }
}

impl<'a> ExecutorExt for StarknetVMProcessor<'a> {
    fn simulate(
        &self,
        transactions: Vec<ExecutableTxWithHash>,
        flags: SimulationFlag,
    ) -> Vec<ResultAndStates> {
        self.simulate_with(transactions, &flags, |(tx, res)| {
            let result = match res {
                Ok((info, fee)) => {
                    // get the trace and receipt from the execution info
                    let trace = utils::to_exec_info(&info);
                    let receipt = receipt_from_exec_info(&tx, &trace);
                    ExecutionResult::new_success(receipt, trace, fee)
                }
                Err(e) => {
                    info!(target: "executor", "transaction simulation failed: {e}");
                    ExecutionResult::new_failed(e)
                }
            };

            ResultAndStates { result, states: Default::default() }
        })
    }

    fn estimate_fee(
        &self,
        transactions: Vec<ExecutableTxWithHash>,
        flags: SimulationFlag,
    ) -> Vec<Result<TxFeeInfo, ExecutionError>> {
        self.simulate_with(transactions, &flags, |(_, res)| match res {
            Ok((_, fee)) => Ok(fee),
            Err(e) => {
                info!(target: "executor", "fee estimation failed: {e}");
                Err(e)
            }
        })
    }

    fn call(&self, call: EntryPointCall) -> Result<Vec<FieldElement>, ExecutionError> {
        let block_context = &self.block_context;
        let retdata = utils::call(call, &self.state, block_context, 100_000_000)?;
        Ok(retdata)
    }
}

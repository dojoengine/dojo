// Re-export the blockifier crate.
pub use blockifier;

mod error;
mod state;
pub mod utils;

use std::num::NonZeroU128;

use blockifier::blockifier::block::{BlockInfo, GasPrices};
use blockifier::context::BlockContext;
use blockifier::state::cached_state::{self, MutRefState};
use blockifier::state::state_api::StateReader;
use katana_cairo::starknet_api::block::{BlockNumber, BlockTimestamp};
use katana_primitives::block::{ExecutableBlock, GasPrices as KatanaGasPrices, PartialHeader};
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::fee::TxFeeInfo;
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, TxWithHash};
use katana_primitives::Felt;
use katana_provider::traits::state::StateProvider;
use tracing::info;

use self::state::CachedState;
use crate::{
    BlockExecutor, EntryPointCall, ExecutionError, ExecutionFlags, ExecutionOutput,
    ExecutionResult, ExecutionStats, ExecutorExt, ExecutorFactory, ExecutorResult, ResultAndStates,
    StateProviderDb,
};

pub(crate) const LOG_TARGET: &str = "katana::executor::blockifier";

#[derive(Debug)]
pub struct BlockifierFactory {
    cfg: CfgEnv,
    flags: ExecutionFlags,
}

impl BlockifierFactory {
    /// Create a new factory with the given configuration and simulation flags.
    pub fn new(cfg: CfgEnv, flags: ExecutionFlags) -> Self {
        Self { cfg, flags }
    }
}

impl ExecutorFactory for BlockifierFactory {
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
        let flags = self.flags.clone();
        Box::new(StarknetVMProcessor::new(Box::new(state), block_env, cfg_env, flags))
    }

    fn cfg(&self) -> &CfgEnv {
        &self.cfg
    }

    /// Returns the execution flags set by the factory.
    fn execution_flags(&self) -> &ExecutionFlags {
        &self.flags
    }
}

#[derive(Debug)]
pub struct StarknetVMProcessor<'a> {
    block_context: BlockContext,
    state: CachedState<StateProviderDb<'a>>,
    transactions: Vec<(TxWithHash, ExecutionResult)>,
    simulation_flags: ExecutionFlags,
    stats: ExecutionStats,
}

impl<'a> StarknetVMProcessor<'a> {
    pub fn new(
        state: Box<dyn StateProvider + 'a>,
        block_env: BlockEnv,
        cfg_env: CfgEnv,
        simulation_flags: ExecutionFlags,
    ) -> Self {
        let transactions = Vec::new();
        let block_context = utils::block_context_from_envs(&block_env, &cfg_env);
        let state = state::CachedState::new(StateProviderDb::new(state));
        Self { block_context, state, transactions, simulation_flags, stats: Default::default() }
    }

    fn fill_block_env_from_header(&mut self, header: &PartialHeader) {
        let number = BlockNumber(header.number);
        let timestamp = BlockTimestamp(header.timestamp);

        // TODO: should we enforce the gas price to not be 0,
        // as there's a flag to disable gas uasge instead?
        let eth_l1_gas_price =
            NonZeroU128::new(header.l1_gas_prices.eth).unwrap_or(NonZeroU128::new(1).unwrap());
        let strk_l1_gas_price =
            NonZeroU128::new(header.l1_gas_prices.strk).unwrap_or(NonZeroU128::new(1).unwrap());

        // TODO: which values is correct for those one?
        let eth_l1_data_gas_price = eth_l1_gas_price;
        let strk_l1_data_gas_price = strk_l1_gas_price;

        // TODO: @kariy, not sure here if we should add some functions to alter it
        // instead of cloning. Or did I miss a function?
        // https://github.com/starkware-libs/blockifier/blob/a6200402ab635d8a8e175f7f135be5914c960007/crates/blockifier/src/context.rs#L23
        let versioned_constants = self.block_context.versioned_constants().clone();
        let chain_info = self.block_context.chain_info().clone();
        let block_info = BlockInfo {
            block_number: number,
            block_timestamp: timestamp,
            sequencer_address: utils::to_blk_address(header.sequencer_address),
            gas_prices: GasPrices {
                eth_l1_gas_price,
                strk_l1_gas_price,
                eth_l1_data_gas_price,
                strk_l1_data_gas_price,
            },
            use_kzg_da: false,
        };

        self.block_context =
            BlockContext::new(block_info, chain_info, versioned_constants, Default::default());
    }

    fn simulate_with<F, T>(
        &self,
        transactions: Vec<ExecutableTxWithHash>,
        flags: &ExecutionFlags,
        mut op: F,
    ) -> Vec<T>
    where
        F: FnMut(&mut dyn StateReader, (TxWithHash, ExecutionResult)) -> T,
    {
        let block_context = &self.block_context;
        let state = &mut self.state.0.lock().inner;
        let mut state = cached_state::CachedState::new(MutRefState::new(state));

        let mut results = Vec::with_capacity(transactions.len());
        for exec_tx in transactions {
            let tx = TxWithHash::from(&exec_tx);
            let res = utils::transact(&mut state, block_context, flags, exec_tx);
            results.push(op(&mut state, (tx, res)));
        }

        results
    }
}

impl<'a> BlockExecutor<'a> for StarknetVMProcessor<'a> {
    fn execute_block(&mut self, block: ExecutableBlock) -> ExecutorResult<()> {
        self.fill_block_env_from_header(&block.header);
        self.execute_transactions(block.body)?;
        Ok(())
    }

    fn execute_transactions(
        &mut self,
        transactions: Vec<ExecutableTxWithHash>,
    ) -> ExecutorResult<()> {
        let block_context = &self.block_context;
        let flags = &self.simulation_flags;
        let mut state = self.state.0.lock();

        for exec_tx in transactions {
            // Collect class artifacts if its a declare tx
            let class_decl_artifacts = if let ExecutableTx::Declare(tx) = exec_tx.as_ref() {
                let class_hash = tx.class_hash();
                Some((class_hash, tx.compiled_class.clone(), tx.sierra_class.clone()))
            } else {
                None
            };

            let tx = TxWithHash::from(&exec_tx);
            let res = utils::transact(&mut state.inner, block_context, flags, exec_tx);

            match &res {
                ExecutionResult::Success { receipt, trace } => {
                    self.stats.l1_gas_used += receipt.fee().gas_consumed;
                    self.stats.cairo_steps_used +=
                        receipt.resources_used().vm_resources.n_steps as u128;

                    if let Some(reason) = receipt.revert_reason() {
                        info!(target: LOG_TARGET, %reason, "Transaction reverted.");
                    }

                    if let Some((class_hash, compiled, sierra)) = class_decl_artifacts {
                        state.declared_classes.insert(class_hash, (compiled, sierra));
                    }

                    crate::utils::log_resources(&trace.actual_resources);
                }

                ExecutionResult::Failed { error } => {
                    info!(target: LOG_TARGET, %error, "Executing transaction.");
                }
            };

            self.transactions.push((tx, res));
        }

        Ok(())
    }

    fn take_execution_output(&mut self) -> ExecutorResult<ExecutionOutput> {
        let states = utils::state_update_from_cached_state(&self.state);
        let transactions = std::mem::take(&mut self.transactions);
        let stats = std::mem::take(&mut self.stats);
        Ok(ExecutionOutput { stats, states, transactions })
    }

    fn state(&self) -> Box<dyn StateProvider + 'a> {
        Box::new(self.state.clone())
    }

    fn transactions(&self) -> &[(TxWithHash, ExecutionResult)] {
        &self.transactions
    }

    fn block_env(&self) -> BlockEnv {
        let eth_l1_gas_price = self.block_context.block_info().gas_prices.eth_l1_gas_price;
        let strk_l1_gas_price = self.block_context.block_info().gas_prices.strk_l1_gas_price;

        BlockEnv {
            number: self.block_context.block_info().block_number.0,
            timestamp: self.block_context.block_info().block_timestamp.0,
            sequencer_address: utils::to_address(self.block_context.block_info().sequencer_address),
            l1_gas_prices: KatanaGasPrices {
                eth: eth_l1_gas_price.into(),
                strk: strk_l1_gas_price.into(),
            },
            l1_data_gas_prices: KatanaGasPrices {
                eth: self.block_context.block_info().gas_prices.eth_l1_data_gas_price.into(),
                strk: self.block_context.block_info().gas_prices.strk_l1_data_gas_price.into(),
            },
        }
    }
}

impl ExecutorExt for StarknetVMProcessor<'_> {
    fn simulate(
        &self,
        transactions: Vec<ExecutableTxWithHash>,
        flags: ExecutionFlags,
    ) -> Vec<ResultAndStates> {
        self.simulate_with(transactions, &flags, |_, (_, result)| ResultAndStates {
            result,
            states: Default::default(),
        })
    }

    fn estimate_fee(
        &self,
        transactions: Vec<ExecutableTxWithHash>,
        flags: ExecutionFlags,
    ) -> Vec<Result<TxFeeInfo, ExecutionError>> {
        self.simulate_with(transactions, &flags, |_, (_, res)| match res {
            ExecutionResult::Success { receipt, .. } => {
                // if the transaction was reverted, return as error
                if let Some(reason) = receipt.revert_reason() {
                    info!(target: LOG_TARGET, %reason, "Estimating fee.");
                    Err(ExecutionError::TransactionReverted { revert_error: reason.to_string() })
                } else {
                    Ok(receipt.fee().clone())
                }
            }

            ExecutionResult::Failed { error } => {
                info!(target: LOG_TARGET, %error, "Estimating fee.");
                Err(error)
            }
        })
    }

    fn call(&self, call: EntryPointCall) -> Result<Vec<Felt>, ExecutionError> {
        let block_context = &self.block_context;
        let mut state = self.state.0.lock();
        let state = MutRefState::new(&mut state.inner);
        let retdata = utils::call(call, state, block_context, 1_000_000_000)?;
        Ok(retdata)
    }
}

mod output;
mod state;
mod utils;

use blockifier::block_context::BlockContext;
use blockifier::state::cached_state::{self, MutRefState};
use katana_primitives::block::{ExecutableBlock, GasPrices, PartialHeader};
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::receipt::Receipt;
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, TxWithHash};
use katana_primitives::FieldElement;
use katana_provider::traits::state::StateProvider;
use starknet_api::block::{BlockNumber, BlockTimestamp};

use self::state::CachedState;
pub use self::utils::Error;
use crate::{
    abstraction, BlockExecutor, EntryPointCall, ExecutionOutput, ExecutorResult, SimulationFlag,
    StateProviderDb, TransactionExecutionOutput, TransactionExecutor,
};

#[derive(Debug)]
pub struct BlockifierFactory {
    cfg: CfgEnv,
    flags: SimulationFlag,
}

impl BlockifierFactory {
    /// Create a new factory with the given configuration and simulation flags.
    pub fn new(cfg: CfgEnv, flags: SimulationFlag) -> Self {
        Self { cfg, flags }
    }
}

impl abstraction::ExecutorFactory for BlockifierFactory {
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
}

pub struct StarknetVMProcessor<'a> {
    block_context: BlockContext,
    state: CachedState<StateProviderDb<'a>>,
    transactions: Vec<(TxWithHash, Option<Receipt>)>,
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
        let state = state::CachedState::new(StateProviderDb(state));
        Self { block_context, state, transactions, simulation_flags }
    }

    fn fill_block_env_from_header(&mut self, header: &PartialHeader) {
        // TODO: include block number in partial header
        let number = BlockNumber(0);
        let timestamp = BlockTimestamp(header.timestamp);
        let eth_l1_gas_price = header.gas_prices.eth as u128;
        let strk_l1_gas_price = header.gas_prices.strk as u128;

        self.block_context.block_info.block_number = number;
        self.block_context.block_info.block_timestamp = timestamp;
        self.block_context.block_info.gas_prices.eth_l1_gas_price = eth_l1_gas_price;
        self.block_context.block_info.gas_prices.strk_l1_gas_price = strk_l1_gas_price;
        self.block_context.block_info.sequencer_address = header.sequencer_address.into();
    }
}

impl<'a> abstraction::TransactionExecutor for StarknetVMProcessor<'a> {
    fn execute(
        &mut self,
        tx: ExecutableTxWithHash,
    ) -> ExecutorResult<Box<dyn TransactionExecutionOutput>> {
        let state = &self.state;
        let block_context = &self.block_context;
        let flags = &self.simulation_flags;

        let class_declaration_artifacts = if let ExecutableTx::Declare(tx) = tx.as_ref() {
            let class_hash = tx.class_hash();
            Some((class_hash, tx.compiled_class.clone(), tx.sierra_class.clone()))
        } else {
            None
        };

        let tx_ = TxWithHash::from(&tx);
        let res = utils::transact(tx, &mut state.0.write().inner, block_context, flags)?;

        let receipt = res.receipt(tx_.as_ref());
        self.transactions.push((tx_, Some(receipt)));

        if let Some((class_hash, compiled_class, sierra_class)) = class_declaration_artifacts {
            state.0.write().declared_classes.insert(class_hash, (compiled_class, sierra_class));
        }

        Ok(Box::new(res))
    }

    fn simulate(
        &self,
        tx: ExecutableTxWithHash,
        flags: SimulationFlag,
    ) -> ExecutorResult<Box<dyn TransactionExecutionOutput>> {
        let block_context = &self.block_context;

        let state = &mut self.state.0.write().inner;
        let mut state = cached_state::CachedState::new(MutRefState::new(state), Default::default());

        let res = utils::transact(tx, &mut state, block_context, &flags)?;
        Ok(Box::new(res))
    }

    fn call(&self, call: EntryPointCall, initial_gas: u128) -> ExecutorResult<Vec<FieldElement>> {
        let block_context = &self.block_context;

        let state = &mut self.state.0.write().inner;
        let mut state = cached_state::CachedState::new(MutRefState::new(state), Default::default());

        let res = utils::call(call, &mut state, block_context, initial_gas)?;

        let retdata = res.execution.retdata.0;
        let retdata = retdata.into_iter().map(|f| f.into()).collect::<Vec<FieldElement>>();

        Ok(retdata)
    }
}

impl<'a> abstraction::BlockExecutor<'a> for StarknetVMProcessor<'a> {
    fn execute_block(&mut self, block: ExecutableBlock) -> ExecutorResult<()> {
        self.fill_block_env_from_header(&block.header);

        for tx in block.body {
            let _ = self.execute(tx)?;
        }

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

    fn transactions(&self) -> &[(TxWithHash, Option<Receipt>)] {
        &self.transactions
    }

    fn block_env(&self) -> BlockEnv {
        BlockEnv {
            number: self.block_context.block_info.block_number.0,
            timestamp: self.block_context.block_info.block_timestamp.0,
            sequencer_address: self.block_context.block_info.sequencer_address.into(),
            l1_gas_prices: GasPrices {
                eth: self.block_context.block_info.gas_prices.eth_l1_gas_price as u64,
                strk: self.block_context.block_info.gas_prices.strk_l1_gas_price as u64,
            },
        }
    }
}

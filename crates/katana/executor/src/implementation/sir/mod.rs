mod output;
mod state;
mod utils;

use std::collections::HashMap;
use std::sync::Arc;

use katana_primitives::block::{ExecutableBlock, PartialHeader};
use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_primitives::contract::{ContractAddress, StorageKey, StorageValue};
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::receipt::Receipt;
use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, TxWithHash};
use katana_primitives::FieldElement;
use katana_provider::traits::state::StateProvider;
use sir::definitions::block_context::{
    BlockContext, FeeTokenAddresses, GasPrices, StarknetOsConfig,
};
use sir::state::contract_class_cache::PermanentContractClassCache;
use sir::state::{cached_state, BlockInfo};

use self::state::CachedState;
pub use self::utils::Error;
use crate::abstraction::{
    BlockExecutor, ExecutionOutput, ExecutorFactory, ExecutorResult, SimulationFlag,
    StateProviderDb, TransactionExecutionOutput, TransactionExecutor,
};
use crate::EntryPointCall;

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
    state: Arc<CachedState<StateProviderDb<'a>, PermanentContractClassCache>>,
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
        let chain_id = utils::to_sir_felt(&cfg_env.chain_id.id());
        let fee_token_addreses = FeeTokenAddresses::new(
            utils::to_sir_address(&cfg_env.fee_token_addresses.eth),
            utils::to_sir_address(&cfg_env.fee_token_addresses.strk),
        );

        let block_info = BlockInfo {
            block_number: block_env.number,
            block_timestamp: block_env.timestamp,
            sequencer_address: utils::to_sir_address(&block_env.sequencer_address),
            gas_price: GasPrices {
                eth_l1_gas_price: block_env.l1_gas_prices.eth as u128,
                strk_l1_gas_price: block_env.l1_gas_prices.strk as u128,
            },
        };

        let block_context = BlockContext::new(
            StarknetOsConfig::new(chain_id, fee_token_addreses, GasPrices::default()),
            Default::default(),
            Default::default(),
            cfg_env.vm_resource_fee_cost.clone(),
            cfg_env.invoke_tx_max_n_steps as u64,
            cfg_env.validate_max_n_steps as u64,
            block_info,
            Default::default(),
            false,
        );

        let contract_class_cache = PermanentContractClassCache::default();
        let state = Arc::new(CachedState::new(StateProviderDb(state), contract_class_cache));
        let executed_txs = Vec::new();

        Self { block_context, state, transactions: executed_txs, simulation_flags }
    }

    fn fill_block_env_from_header(&mut self, header: &PartialHeader) {
        let number = header.number;
        let timestamp = header.timestamp;
        let sequencer_address = utils::to_sir_address(&header.sequencer_address);
        let eth_l1_gas_price = header.gas_prices.eth as u128;
        let strk_l1_gas_price = header.gas_prices.strk as u128;

        self.block_context.block_info_mut().block_number = number;
        self.block_context.block_info_mut().block_timestamp = timestamp;
        self.block_context.block_info_mut().sequencer_address = sequencer_address;
        self.block_context.block_info_mut().gas_price.eth_l1_gas_price = eth_l1_gas_price;
        self.block_context.block_info_mut().gas_price.strk_l1_gas_price = strk_l1_gas_price;
    }
}

impl<'a> TransactionExecutor for StarknetVMProcessor<'a> {
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

        let gas = 0;
        let res = utils::transact(tx, &mut state.0.write().inner, block_context, gas, flags)?;

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
        let state = &self.state.0.read().inner;

        let state_reader = Arc::new(state);
        let contract_classes = Arc::new(PermanentContractClassCache::default());
        let mut state = cached_state::CachedState::new(state_reader, contract_classes);

        let block_context = &self.block_context;

        let gas = 0;
        let res = utils::transact(tx, &mut state, block_context, gas, &flags)?;

        Ok(Box::new(res))
    }

    fn call(&self, call: EntryPointCall, initial_gas: u128) -> ExecutorResult<Vec<FieldElement>> {
        let block_context = &self.block_context;

        let state_reader = Arc::clone(&self.state);
        let contract_classes = Arc::new(PermanentContractClassCache::default());
        let mut state = cached_state::CachedState::new(state_reader, contract_classes);

        let res = utils::call(call, &mut state, block_context, initial_gas)?;

        let info = res.call_info.expect("should exist in call result");
        let retdata = info.retdata.iter().map(utils::to_felt).collect();

        Ok(retdata)
    }
}

impl<'a> BlockExecutor<'a> for StarknetVMProcessor<'a> {
    fn execute_block(&mut self, block: ExecutableBlock) -> ExecutorResult<()> {
        self.fill_block_env_from_header(&block.header);

        for tx in block.body {
            let _ = self.execute(tx)?;
        }

        Ok(())
    }

    fn take_execution_output(&mut self) -> ExecutorResult<ExecutionOutput> {
        let transactions = std::mem::take(&mut self.transactions);
        let state = &mut self.state.0.write();

        let state_changes = std::mem::take(state.inner.cache_mut());
        let state_diffs = utils::state_diff_from_state_cache(state_changes);
        let compiled_classes = std::mem::take(&mut state.declared_classes);

        let nonce_updates: HashMap<ContractAddress, FieldElement> = state_diffs
            .address_to_nonce()
            .iter()
            .map(|(k, v)| (utils::to_address(k), utils::to_felt(v)))
            .collect();

        let declared_classes: HashMap<ClassHash, CompiledClassHash> = state_diffs
            .class_hash_to_compiled_class()
            .iter()
            .map(|(k, v)| (utils::to_class_hash(k), utils::to_class_hash(v)))
            .collect();

        let contract_updates: HashMap<ContractAddress, ClassHash> = state_diffs
            .address_to_class_hash()
            .iter()
            .map(|(k, v)| (utils::to_address(k), utils::to_class_hash(v)))
            .collect();

        let storage_updates: HashMap<ContractAddress, HashMap<StorageKey, StorageValue>> =
            state_diffs
                .storage_updates()
                .iter()
                .map(|(k, v)| {
                    let k = utils::to_address(k);
                    let v = v.iter().map(|(k, v)| (utils::to_felt(k), utils::to_felt(v))).collect();
                    (k, v)
                })
                .collect();

        let total_classes = declared_classes.len();
        let mut declared_compiled_classes = HashMap::with_capacity(total_classes);
        let mut declared_sierra_classes = HashMap::with_capacity(total_classes);

        for (hash, (compiled, sierra)) in compiled_classes {
            declared_compiled_classes.insert(hash, compiled);
            if let Some(sierra) = sierra {
                declared_sierra_classes.insert(hash, sierra);
            }
        }

        let state_updates = StateUpdatesWithDeclaredClasses {
            declared_sierra_classes,
            declared_compiled_classes,
            state_updates: StateUpdates {
                nonce_updates,
                storage_updates,
                contract_updates,
                declared_classes,
            },
        };

        Ok(ExecutionOutput { states: state_updates, transactions })
    }

    fn state(&self) -> Box<dyn StateProvider + 'a> {
        Box::new(self.state.clone())
    }

    fn transactions(&self) -> &[(TxWithHash, Option<Receipt>)] {
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
                eth: self.block_context.block_info().gas_price.eth_l1_gas_price as u64,
                strk: self.block_context.block_info().gas_price.strk_l1_gas_price as u64,
            },
        }
    }
}

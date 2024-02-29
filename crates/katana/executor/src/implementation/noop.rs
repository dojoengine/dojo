use katana_primitives::block::ExecutableBlock;
use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, FlattenedSierraClass};
use katana_primitives::contract::{ContractAddress, Nonce, StorageKey, StorageValue};
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::receipt::{InvokeTxReceipt, Receipt};
use katana_primitives::transaction::{ExecutableTxWithHash, Tx, TxWithHash};
use katana_primitives::FieldElement;
use katana_provider::traits::contract::ContractClassProvider;
use katana_provider::traits::state::StateProvider;
use katana_provider::ProviderResult;

use crate::{
    BlockExecutor, EntryPointCall, ExecutionOutput, ExecutorFactory, ExecutorResult,
    SimulationFlag, TransactionExecutionOutput, TransactionExecutor,
};

/// A no-op executor factory. Creates an executor that does nothing.
#[derive(Debug, Default)]
pub struct NoopExecutorFactory {
    cfg: CfgEnv,
}

impl NoopExecutorFactory {
    /// Create a new no-op executor factory.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ExecutorFactory for NoopExecutorFactory {
    fn with_state<'a, P>(&self, state: P) -> Box<dyn BlockExecutor<'a> + 'a>
    where
        P: StateProvider + 'a,
    {
        let _ = state;
        Box::<NoopExecutor>::default()
    }

    fn with_state_and_block_env<'a, P>(
        &self,
        state: P,
        block_env: BlockEnv,
    ) -> Box<dyn BlockExecutor<'a> + 'a>
    where
        P: StateProvider + 'a,
    {
        let _ = state;
        let _ = block_env;
        Box::new(NoopExecutor { block_env })
    }

    fn cfg(&self) -> &CfgEnv {
        &self.cfg
    }
}

#[derive(Debug, Default)]
struct NoopExecutor {
    block_env: BlockEnv,
}

impl TransactionExecutor for NoopExecutor {
    fn execute(
        &mut self,
        tx: ExecutableTxWithHash,
    ) -> ExecutorResult<Box<dyn TransactionExecutionOutput>> {
        let _ = tx;
        Ok(Box::new(NoopTransactionExecutionOutput))
    }

    fn simulate(
        &self,
        tx: ExecutableTxWithHash,
        flags: SimulationFlag,
    ) -> ExecutorResult<Box<dyn TransactionExecutionOutput>> {
        let _ = tx;
        let _ = flags;
        Ok(Box::new(NoopTransactionExecutionOutput))
    }

    fn call(&self, call: EntryPointCall, initial_gas: u128) -> ExecutorResult<Vec<FieldElement>> {
        let _ = call;
        let _ = initial_gas;
        Ok(vec![])
    }
}

impl<'a> BlockExecutor<'a> for NoopExecutor {
    fn execute_block(&mut self, block: ExecutableBlock) -> ExecutorResult<()> {
        let _ = block;
        Ok(())
    }

    fn take_execution_output(&mut self) -> ExecutorResult<ExecutionOutput> {
        Ok(ExecutionOutput::default())
    }

    fn state(&self) -> Box<dyn StateProvider + 'a> {
        Box::new(NoopStateProvider)
    }

    fn transactions(&self) -> &[(TxWithHash, Option<Receipt>)] {
        &[]
    }

    fn block_env(&self) -> BlockEnv {
        self.block_env.clone()
    }
}

struct NoopTransactionExecutionOutput;

impl TransactionExecutionOutput for NoopTransactionExecutionOutput {
    fn receipt(&self, tx: &Tx) -> Receipt {
        let _ = tx;
        Receipt::Invoke(InvokeTxReceipt::default())
    }

    fn actual_fee(&self) -> u128 {
        0
    }

    fn gas_used(&self) -> u128 {
        0
    }

    fn revert_error(&self) -> Option<&str> {
        None
    }
}

struct NoopStateProvider;

impl ContractClassProvider for NoopStateProvider {
    fn class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        let _ = hash;
        Ok(None)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        let _ = hash;
        Ok(None)
    }

    fn sierra_class(&self, hash: ClassHash) -> ProviderResult<Option<FlattenedSierraClass>> {
        let _ = hash;
        Ok(None)
    }
}

impl StateProvider for NoopStateProvider {
    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<ClassHash>> {
        let _ = address;
        Ok(None)
    }

    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        let _ = address;
        Ok(None)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        let _ = address;
        let _ = storage_key;
        Ok(None)
    }
}

use katana_primitives::block::ExecutableBlock;
use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, FlattenedSierraClass};
use katana_primitives::contract::{ContractAddress, Nonce, StorageKey, StorageValue};
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::receipt::Receipt;
use katana_primitives::state::StateUpdatesWithDeclaredClasses;
use katana_primitives::transaction::{ExecutableTxWithHash, Tx, TxWithHash};
use katana_primitives::FieldElement;
use katana_provider::traits::contract::ContractClassProvider;
use katana_provider::traits::state::StateProvider;
use katana_provider::ProviderResult;

#[derive(Debug, thiserror::Error)]
pub enum ExecutorError {
    #[error("Insufficient balance")]
    InsufficientBalance,

    #[error("Requested entry point was not found")]
    EntryPointNotFound,

    #[error("Contract ({0}) is not deployed")]
    ContractNotDeployed(ContractAddress),

    #[error("Invalid transaction nonce. Expected: {expected}, got: {actual}")]
    InvalidTransactionNonce {
        /// The actual nonce
        actual: Nonce,
        /// The expected nonce
        expected: Nonce,
    },

    #[error("Invalid transaction signature")]
    InvalidSignature,

    #[error(transparent)]
    Other(Box<dyn std::error::Error + Send>),
}

pub type ExecutorResult<T> = Result<T, ExecutorError>;

/// Transaction execution simulation flags.
///
/// These flags can be used to control the behavior of the transaction execution, such as skipping
/// the transaction execution or validation, or ignoring the maximum fee when validating the
/// transaction.
#[derive(Debug, Clone, Default)]
pub struct SimulationFlag {
    /// Skip the transaction execution.
    pub skip_execute: bool,
    /// Skip the transaction validation.
    pub skip_validate: bool,
    /// Skip checking nonce when validating the transaction.
    pub skip_nonce_check: bool,
    /// Skip the fee transfer after the transaction execution.
    pub skip_fee_transfer: bool,
    /// Ignore the maximum fee when validating the transaction.
    pub ignore_max_fee: bool,
}

/// The output of a executor after a series of executions.
#[derive(Debug, Default)]
pub struct ExecutionOutput {
    /// The state updates produced by the executions.
    pub states: StateUpdatesWithDeclaredClasses,
    /// The transactions that have been executed.
    pub transactions: Vec<(TxWithHash, Option<Receipt>)>,
}

#[derive(Debug)]
pub struct EntryPointCall {
    /// The address of the contract whose function you're calling.
    pub contract_address: ContractAddress,
    /// The input to the function.
    pub calldata: Vec<FieldElement>,
    /// The contract function name.
    pub entry_point_selector: FieldElement,
}

/// A type that can create [BlockExecutor] instance.
pub trait ExecutorFactory: Send + Sync + 'static {
    /// Construct a new [BlockExecutor] with the given state.
    fn with_state<'a, P>(&self, state: P) -> Box<dyn BlockExecutor<'a> + 'a>
    where
        P: StateProvider + 'a;

    /// Construct a new [BlockExecutor] with the given state and block environment values.
    fn with_state_and_block_env<'a, P>(
        &self,
        state: P,
        block_env: BlockEnv,
    ) -> Box<dyn BlockExecutor<'a> + 'a>
    where
        P: StateProvider + 'a;

    /// Returns the configuration environment of the factory.
    fn cfg(&self) -> &CfgEnv;
}

/// An executor that can execute a block of transactions.
pub trait BlockExecutor<'a>: TransactionExecutor + Send + Sync {
    /// Executes the given block.
    fn execute_block(&mut self, block: ExecutableBlock) -> ExecutorResult<()>;

    /// Takes the output state of the executor.
    fn take_execution_output(&mut self) -> ExecutorResult<ExecutionOutput>;

    /// Returns the current state of the executor.
    fn state(&self) -> Box<dyn StateProvider + 'a>;

    /// Returns the transactions that have been executed.
    fn transactions(&self) -> &[(TxWithHash, Option<Receipt>)];

    /// Returns the current block environment of the executor.
    fn block_env(&self) -> BlockEnv;
}

/// Type that can execute transactions.
pub trait TransactionExecutor {
    /// Executes the given transaction and returns the output.
    fn execute(
        &mut self,
        tx: ExecutableTxWithHash,
    ) -> ExecutorResult<Box<dyn TransactionExecutionOutput>>;

    /// Executes the given transaction according to the simulation flags and returns the output,
    /// without committing to the state.
    fn simulate(
        &self,
        tx: ExecutableTxWithHash,
        flags: SimulationFlag,
    ) -> ExecutorResult<Box<dyn TransactionExecutionOutput>>;

    /// TODO: make `initial_gas` as `ExecutorFactory` responsibility
    fn call(&self, call: EntryPointCall, initial_gas: u128) -> ExecutorResult<Vec<FieldElement>>;
}

/// The output of a transaction execution.
pub trait TransactionExecutionOutput {
    /// Retrieves the receipt from the transaction execution ouput.
    fn receipt(&self, tx: &Tx) -> Receipt;

    /// The transaction fee that was actually paid.
    fn actual_fee(&self) -> u128;

    /// The total gas used by the transaction.
    fn gas_used(&self) -> u128;

    /// The error message if the transaction execution reverted, otherwise the value is `None`.
    fn revert_error(&self) -> Option<&str>;
}

/// A wrapper around a boxed [StateProvider] for implementing the executor's own state reader
/// traits.
pub(crate) struct StateProviderDb<'a>(pub(crate) Box<dyn StateProvider + 'a>);

impl<'a> ContractClassProvider for StateProviderDb<'a> {
    fn class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        self.0.class(hash)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        self.0.compiled_class_hash_of_class_hash(hash)
    }

    fn sierra_class(&self, hash: ClassHash) -> ProviderResult<Option<FlattenedSierraClass>> {
        self.0.sierra_class(hash)
    }
}

impl<'a> StateProvider for StateProviderDb<'a> {
    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<ClassHash>> {
        self.0.class_hash_of_contract(address)
    }

    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        self.0.nonce(address)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        self.0.storage(address, storage_key)
    }
}

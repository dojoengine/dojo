mod error;
mod executor;

pub use error::*;
pub use executor::*;
use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, FlattenedSierraClass};
use katana_primitives::contract::{ContractAddress, Nonce, StorageKey, StorageValue};
use katana_primitives::receipt::Receipt;
use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};
use katana_primitives::trace::TxExecInfo;
use katana_primitives::transaction::TxWithHash;
use katana_primitives::Felt;
use katana_provider::traits::contract::ContractClassProvider;
use katana_provider::traits::state::StateProvider;
use katana_provider::ProviderResult;

pub type ExecutorResult<T> = Result<T, error::ExecutorError>;

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

impl SimulationFlag {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enables the skip execution flag.
    pub fn skip_execute(mut self) -> Self {
        self.skip_execute = true;
        self
    }

    /// Enables the skip validation flag.
    pub fn skip_validate(mut self) -> Self {
        self.skip_validate = true;
        self
    }

    /// Enables the skip nonce check flag.
    pub fn skip_nonce_check(mut self) -> Self {
        self.skip_nonce_check = true;
        self
    }

    /// Enables the skip fee transfer flag.
    pub fn skip_fee_transfer(mut self) -> Self {
        self.skip_fee_transfer = true;
        self
    }

    /// Enables the ignore max fee flag.
    pub fn ignore_max_fee(mut self) -> Self {
        self.ignore_max_fee = true;
        self
    }
}

/// Stats about the transactions execution.
#[derive(Debug, Clone, Default)]
pub struct ExecutionStats {
    /// The total gas used.
    pub l1_gas_used: u128,
    /// The total cairo steps used.
    pub cairo_steps_used: u128,
}

/// The output of a executor after a series of executions.
#[derive(Debug, Default)]
pub struct ExecutionOutput {
    /// Statistics throughout the executions process.
    pub stats: ExecutionStats,
    /// The state updates produced by the executions.
    pub states: StateUpdatesWithDeclaredClasses,
    /// The transactions that have been executed.
    pub transactions: Vec<(TxWithHash, ExecutionResult)>,
}

#[derive(Debug)]
pub struct EntryPointCall {
    /// The address of the contract whose function you're calling.
    pub contract_address: ContractAddress,
    /// The input to the function.
    pub calldata: Vec<Felt>,
    /// The contract function name.
    pub entry_point_selector: Felt,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum ExecutionResult {
    Success { receipt: Receipt, trace: TxExecInfo },
    Failed { error: ExecutionError },
}

impl ExecutionResult {
    pub fn new_success(receipt: Receipt, trace: TxExecInfo) -> Self {
        ExecutionResult::Success { receipt, trace }
    }

    pub fn new_failed(error: impl Into<ExecutionError>) -> Self {
        ExecutionResult::Failed { error: error.into() }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, ExecutionResult::Success { .. })
    }

    pub fn is_failed(&self) -> bool {
        !self.is_success()
    }

    pub fn receipt(&self) -> Option<&Receipt> {
        match self {
            ExecutionResult::Success { receipt, .. } => Some(receipt),
            _ => None,
        }
    }

    pub fn trace(&self) -> Option<&TxExecInfo> {
        match self {
            ExecutionResult::Success { trace, .. } => Some(trace),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResultAndStates {
    pub result: ExecutionResult,
    pub states: StateUpdates,
}

/// A wrapper around a boxed [StateProvider] for implementing the executor's own state reader
/// traits.
#[derive(Debug)]
pub struct StateProviderDb<'a>(Box<dyn StateProvider + 'a>);

impl<'a> StateProviderDb<'a> {
    pub fn new(provider: Box<dyn StateProvider + 'a>) -> Self {
        Self(provider)
    }
}

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

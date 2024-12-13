mod error;
mod executor;

pub use error::*;
pub use executor::*;
use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, ContractClass};
use katana_primitives::contract::{ContractAddress, Nonce, StorageKey, StorageValue};
use katana_primitives::receipt::Receipt;
use katana_primitives::state::{StateUpdates, StateUpdatesWithClasses};
use katana_primitives::trace::TxExecInfo;
use katana_primitives::transaction::TxWithHash;
use katana_primitives::Felt;
use katana_provider::traits::contract::ContractClassProvider;
use katana_provider::traits::state::{StateProofProvider, StateProvider};
use katana_provider::ProviderResult;
use katana_trie::MultiProof;

pub type ExecutorResult<T> = Result<T, error::ExecutorError>;

/// Transaction execution simulation flags.
///
/// These flags can be used to control the behavior of the transaction execution, such as skipping
/// the transaction validation, or ignoring any fee related checks.
#[derive(Debug, Clone)]
pub struct ExecutionFlags {
    /// Determine whether to perform the transaction sender's account validation logic.
    account_validation: bool,
    /// Determine whether to perform fee related checks and operations ie., fee transfer.
    fee: bool,
    /// Determine whether to perform transaction's sender nonce check.
    nonce_check: bool,
}

impl Default for ExecutionFlags {
    fn default() -> Self {
        Self { account_validation: true, fee: true, nonce_check: true }
    }
}

impl ExecutionFlags {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to enable or disable the account validation.
    pub fn with_account_validation(mut self, enable: bool) -> Self {
        self.account_validation = enable;
        self
    }

    /// Set whether to enable or disable the fee related operations.
    pub fn with_fee(mut self, enable: bool) -> Self {
        self.fee = enable;
        self
    }

    /// Set whether to enable or disable the nonce check.
    pub fn with_nonce_check(mut self, enable: bool) -> Self {
        self.nonce_check = enable;
        self
    }

    /// Returns whether the account validation is enabled.
    pub fn account_validation(&self) -> bool {
        self.account_validation
    }

    /// Returns whether the fee related operations are enabled.
    pub fn fee(&self) -> bool {
        self.fee
    }

    /// Returns whether the nonce check is enabled.
    pub fn nonce_check(&self) -> bool {
        self.nonce_check
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
    pub states: StateUpdatesWithClasses,
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
    /// Creates a new successful execution result.
    pub fn new_success(receipt: Receipt, trace: TxExecInfo) -> Self {
        ExecutionResult::Success { receipt, trace }
    }

    /// Creates a new failed execution result with the given error.
    pub fn new_failed(error: impl Into<ExecutionError>) -> Self {
        ExecutionResult::Failed { error: error.into() }
    }

    /// Returns `true` if the execution was successful.
    pub fn is_success(&self) -> bool {
        matches!(self, ExecutionResult::Success { .. })
    }

    /// Returns `true` if the execution failed.
    pub fn is_failed(&self) -> bool {
        !self.is_success()
    }

    /// Returns the receipt of the execution if it was successful. Otherwise, returns `None`.
    pub fn receipt(&self) -> Option<&Receipt> {
        match self {
            ExecutionResult::Success { receipt, .. } => Some(receipt),
            _ => None,
        }
    }

    /// Returns the execution info if it was successful. Otherwise, returns `None`.
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
    fn class(&self, hash: ClassHash) -> ProviderResult<Option<ContractClass>> {
        self.0.class(hash)
    }

    fn compiled_class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        self.0.compiled_class(hash)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        self.0.compiled_class_hash_of_class_hash(hash)
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

impl<'a> StateProofProvider for StateProviderDb<'a> {
    fn class_multiproof(&self, classes: Vec<ClassHash>) -> ProviderResult<MultiProof> {
        self.0.class_multiproof(classes)
    }

    fn contract_multiproof(&self, addresses: Vec<ContractAddress>) -> ProviderResult<MultiProof> {
        self.0.contract_multiproof(addresses)
    }

    fn storage_multiproof(
        &self,
        address: ContractAddress,
        key: Vec<StorageKey>,
    ) -> ProviderResult<MultiProof> {
        self.0.storage_multiproof(address, key)
    }
}

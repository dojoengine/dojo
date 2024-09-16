use katana_primitives::block::ExecutableBlock;
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::fee::TxFeeInfo;
use katana_primitives::transaction::{ExecutableTxWithHash, TxWithHash};
use katana_primitives::Felt;
use katana_provider::traits::state::StateProvider;

use crate::{
    EntryPointCall, ExecutionError, ExecutionOutput, ExecutionResult, ExecutorResult,
    ResultAndStates, SimulationFlag,
};

/// A type that can create [BlockExecutor] instance.
pub trait ExecutorFactory: Send + Sync + 'static + core::fmt::Debug {
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

    /// Returns the execution flags set by the factory.
    fn execution_flags(&self) -> &SimulationFlag;
}

/// An executor that can execute a block of transactions.
pub trait BlockExecutor<'a>: ExecutorExt + Send + Sync + core::fmt::Debug {
    /// Executes the given block.
    fn execute_block(&mut self, block: ExecutableBlock) -> ExecutorResult<()>;

    fn execute_transactions(
        &mut self,
        transactions: Vec<ExecutableTxWithHash>,
    ) -> ExecutorResult<()>;

    /// Takes the output state of the executor.
    fn take_execution_output(&mut self) -> ExecutorResult<ExecutionOutput>;

    /// Returns the current state of the executor.
    fn state(&self) -> Box<dyn StateProvider + 'a>;

    /// Returns the transactions that have been executed.
    fn transactions(&self) -> &[(TxWithHash, ExecutionResult)];

    /// Returns the current block environment of the executor.
    fn block_env(&self) -> BlockEnv;
}

pub trait ExecutorExt {
    /// Simulate the given transactions and return the results of each transaction.
    fn simulate(
        &self,
        transactions: Vec<ExecutableTxWithHash>,
        flags: SimulationFlag,
    ) -> Vec<ResultAndStates>;

    /// Get the fee estimation for the given transactions.
    fn estimate_fee(
        &self,
        transactions: Vec<ExecutableTxWithHash>,
        flags: SimulationFlag,
    ) -> Vec<Result<TxFeeInfo, ExecutionError>>;

    /// Perform a contract entry point call and return the output.
    fn call(&self, call: EntryPointCall) -> Result<Vec<Felt>, ExecutionError>;
}

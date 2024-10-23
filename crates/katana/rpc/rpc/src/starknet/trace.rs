use jsonrpsee::core::{async_trait, RpcResult};
use katana_executor::{ExecutionResult, ExecutorFactory, ResultAndStates};
use katana_primitives::block::{BlockHashOrNumber, BlockIdOrTag};
use katana_primitives::fee::TxFeeInfo;
use katana_primitives::trace::{BuiltinCounters, TxExecInfo};
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, TxHash, TxType};
use katana_provider::traits::block::{BlockNumberProvider, BlockProvider};
use katana_provider::traits::transaction::{TransactionTraceProvider, TransactionsProviderExt};
use katana_rpc_api::starknet::StarknetTraceApiServer;
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::trace::FunctionInvocation;
use katana_rpc_types::transaction::BroadcastedTx;
use katana_rpc_types::{FeeEstimate, SimulationFlag};
use starknet::core::types::{
    BlockTag, ComputationResources, DataAvailabilityResources, DataResources,
    DeclareTransactionTrace, DeployAccountTransactionTrace, ExecuteInvocation, ExecutionResources,
    InvokeTransactionTrace, L1HandlerTransactionTrace, PriceUnit, RevertedInvocation,
    SimulatedTransaction, TransactionTrace, TransactionTraceWithHash,
};

use super::StarknetApi;

impl<EF: ExecutorFactory> StarknetApi<EF> {
    fn simulate_txs(
        &self,
        block_id: BlockIdOrTag,
        transactions: Vec<BroadcastedTx>,
        simulation_flags: Vec<SimulationFlag>,
    ) -> Result<Vec<SimulatedTransaction>, StarknetApiError> {
        let chain_id = self.inner.backend.chain_spec.id;

        let executables = transactions
            .into_iter()
            .map(|tx| {
                let tx = match tx {
                    BroadcastedTx::Invoke(tx) => {
                        let is_query = tx.is_query();
                        ExecutableTxWithHash::new_query(
                            ExecutableTx::Invoke(tx.into_tx_with_chain_id(chain_id)),
                            is_query,
                        )
                    }
                    BroadcastedTx::Declare(tx) => {
                        let is_query = tx.is_query();
                        ExecutableTxWithHash::new_query(
                            ExecutableTx::Declare(
                                tx.try_into_tx_with_chain_id(chain_id)
                                    .map_err(|_| StarknetApiError::InvalidContractClass)?,
                            ),
                            is_query,
                        )
                    }
                    BroadcastedTx::DeployAccount(tx) => {
                        let is_query = tx.is_query();
                        ExecutableTxWithHash::new_query(
                            ExecutableTx::DeployAccount(tx.into_tx_with_chain_id(chain_id)),
                            is_query,
                        )
                    }
                };
                Result::<ExecutableTxWithHash, StarknetApiError>::Ok(tx)
            })
            .collect::<Result<Vec<_>, _>>()?;

        // If the node is run with transaction validation disabled, then we should not validate
        // even if the `SKIP_VALIDATE` flag is not set.
        let should_validate = !(simulation_flags.contains(&SimulationFlag::SkipValidate)
            || self.inner.backend.executor_factory.execution_flags().skip_validate);

        // If the node is run with fee charge disabled, then we should disable charing fees even
        // if the `SKIP_FEE_CHARGE` flag is not set.
        let should_skip_fee = !(simulation_flags.contains(&SimulationFlag::SkipFeeCharge)
            || self.inner.backend.executor_factory.execution_flags().skip_fee_transfer);

        let flags = katana_executor::SimulationFlag {
            skip_validate: !should_validate,
            skip_fee_transfer: !should_skip_fee,
            ..Default::default()
        };

        // get the state and block env at the specified block for execution
        let state = self.state(&block_id)?;
        let env = self.block_env_at(&block_id)?;

        // create the executor
        let executor = self.inner.backend.executor_factory.with_state_and_block_env(state, env);
        let results = executor.simulate(executables, flags);

        let mut simulated = Vec::with_capacity(results.len());
        for (i, ResultAndStates { result, .. }) in results.into_iter().enumerate() {
            match result {
                ExecutionResult::Success { trace, receipt } => {
                    let transaction_trace = to_rpc_trace(trace);
                    let fee_estimation = to_rpc_fee_estimate(receipt.fee().clone());
                    let value = SimulatedTransaction { transaction_trace, fee_estimation };
                    simulated.push(value)
                }

                ExecutionResult::Failed { error } => {
                    let error = StarknetApiError::TransactionExecutionError {
                        transaction_index: i,
                        execution_error: error.to_string(),
                    };
                    return Err(error);
                }
            }
        }

        Ok(simulated)
    }

    fn block_traces(
        &self,
        block_id: BlockIdOrTag,
    ) -> Result<Vec<TransactionTraceWithHash>, StarknetApiError> {
        use StarknetApiError::BlockNotFound;

        let provider = self.inner.backend.blockchain.provider();

        let block_id: BlockHashOrNumber = match block_id {
            BlockIdOrTag::Tag(BlockTag::Pending) => match self.pending_executor() {
                Some(state) => {
                    let pending_block = state.read();

                    // extract the txs from the pending block
                    let traces = pending_block.transactions().iter().filter_map(|(t, r)| {
                        if let Some(trace) = r.trace() {
                            let transaction_hash = t.hash;
                            let trace_root = to_rpc_trace(trace.clone());
                            Some(TransactionTraceWithHash { transaction_hash, trace_root })
                        } else {
                            None
                        }
                    });

                    return Ok(traces.collect::<Vec<TransactionTraceWithHash>>());
                }

                // if there is no pending block, return the latest block
                None => provider.latest_number()?.into(),
            },
            BlockIdOrTag::Tag(BlockTag::Latest) => provider.latest_number()?.into(),
            BlockIdOrTag::Number(num) => num.into(),
            BlockIdOrTag::Hash(hash) => hash.into(),
        };

        // TODO: we should probably simplify this query
        let indices = provider.block_body_indices(block_id)?.ok_or(BlockNotFound)?;
        let hashes = provider.transaction_hashes_in_range(indices.into())?;
        let traces = provider.transaction_executions_by_block(block_id)?.ok_or(BlockNotFound)?;

        // convert to rpc types
        let traces = traces.into_iter().map(to_rpc_trace);
        let result = hashes
            .into_iter()
            .zip(traces)
            .map(|(h, r)| TransactionTraceWithHash { transaction_hash: h, trace_root: r })
            .collect::<Vec<_>>();

        Ok(result)
    }

    fn trace(&self, tx_hash: TxHash) -> Result<TransactionTrace, StarknetApiError> {
        use StarknetApiError::TxnHashNotFound;

        // Check in the pending block first
        if let Some(state) = self.pending_executor() {
            let pending_block = state.read();
            let tx = pending_block.transactions().iter().find(|(t, _)| t.hash == tx_hash);

            if let Some(trace) = tx.and_then(|(_, res)| res.trace()) {
                return Ok(to_rpc_trace(trace.clone()));
            }
        }

        // If not found in pending block, fallback to the provider
        let provider = self.inner.backend.blockchain.provider();
        let trace = provider.transaction_execution(tx_hash)?.ok_or(TxnHashNotFound)?;

        Ok(to_rpc_trace(trace))
    }
}

#[async_trait]
impl<EF: ExecutorFactory> StarknetTraceApiServer for StarknetApi<EF> {
    async fn trace_transaction(&self, transaction_hash: TxHash) -> RpcResult<TransactionTrace> {
        self.on_io_blocking_task(move |this| Ok(this.trace(transaction_hash)?)).await
    }

    async fn simulate_transactions(
        &self,
        block_id: BlockIdOrTag,
        transactions: Vec<BroadcastedTx>,
        simulation_flags: Vec<SimulationFlag>,
    ) -> RpcResult<Vec<SimulatedTransaction>> {
        self.on_cpu_blocking_task(move |this| {
            Ok(this.simulate_txs(block_id, transactions, simulation_flags)?)
        })
        .await
    }

    async fn trace_block_transactions(
        &self,
        block_id: BlockIdOrTag,
    ) -> RpcResult<Vec<TransactionTraceWithHash>> {
        self.on_io_blocking_task(move |this| Ok(this.block_traces(block_id)?)).await
    }
}

// TODO: move this conversion to katana_rpc_types

fn to_rpc_trace(trace: TxExecInfo) -> TransactionTrace {
    let fee_transfer_invocation =
        trace.fee_transfer_call_info.map(|f| FunctionInvocation::from(f).0);
    let validate_invocation = trace.validate_call_info.map(|f| FunctionInvocation::from(f).0);
    let execute_invocation = trace.execute_call_info.map(|f| FunctionInvocation::from(f).0);
    let revert_reason = trace.revert_error;
    // TODO: compute the state diff
    let state_diff = None;

    let execution_resources = to_rpc_resources(trace.actual_resources.vm_resources);

    match trace.r#type {
        TxType::Invoke => {
            let execute_invocation = if let Some(revert_reason) = revert_reason {
                let invocation = RevertedInvocation { revert_reason };
                ExecuteInvocation::Reverted(invocation)
            } else {
                let invocation = execute_invocation.expect("should exist if not reverted");
                ExecuteInvocation::Success(invocation)
            };

            TransactionTrace::Invoke(InvokeTransactionTrace {
                fee_transfer_invocation,
                execution_resources,
                validate_invocation,
                execute_invocation,
                state_diff,
            })
        }

        TxType::Declare => TransactionTrace::Declare(DeclareTransactionTrace {
            fee_transfer_invocation,
            validate_invocation,
            execution_resources,
            state_diff,
        }),

        TxType::DeployAccount => {
            let constructor_invocation = execute_invocation.expect("should exist if not reverted");
            TransactionTrace::DeployAccount(DeployAccountTransactionTrace {
                fee_transfer_invocation,
                constructor_invocation,
                validate_invocation,
                execution_resources,
                state_diff,
            })
        }

        TxType::L1Handler => {
            let function_invocation = execute_invocation.expect("should exist if not reverted");
            TransactionTrace::L1Handler(L1HandlerTransactionTrace {
                execution_resources,
                function_invocation,
                state_diff,
            })
        }
    }
}

fn to_rpc_resources(resources: katana_primitives::trace::ExecutionResources) -> ExecutionResources {
    let steps = resources.n_steps as u64;
    let memory_holes = resources.n_memory_holes as u64;
    let builtins = BuiltinCounters::from(resources.builtin_instance_counter);

    let data_availability = DataAvailabilityResources { l1_gas: 0, l1_data_gas: 0 };
    let data_resources = DataResources { data_availability };

    let computation_resources = ComputationResources {
        steps,
        memory_holes: Some(memory_holes),
        ecdsa_builtin_applications: builtins.ecdsa(),
        ec_op_builtin_applications: builtins.ec_op(),
        keccak_builtin_applications: builtins.keccak(),
        segment_arena_builtin: builtins.segment_arena(),
        bitwise_builtin_applications: builtins.bitwise(),
        pedersen_builtin_applications: builtins.pedersen(),
        poseidon_builtin_applications: builtins.poseidon(),
        range_check_builtin_applications: builtins.range_check(),
    };

    ExecutionResources { data_resources, computation_resources }
}

fn to_rpc_fee_estimate(fee: TxFeeInfo) -> FeeEstimate {
    FeeEstimate {
        unit: match fee.unit {
            katana_primitives::fee::PriceUnit::Wei => PriceUnit::Wei,
            katana_primitives::fee::PriceUnit::Fri => PriceUnit::Fri,
        },
        gas_price: fee.gas_price.into(),
        overall_fee: fee.overall_fee.into(),
        gas_consumed: fee.gas_consumed.into(),
        data_gas_price: Default::default(),
        data_gas_consumed: Default::default(),
    }
}

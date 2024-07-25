use jsonrpsee::core::{async_trait, Error, RpcResult};
use jsonrpsee::types::error::{CallError, METHOD_NOT_FOUND_CODE};
use jsonrpsee::types::ErrorObject;
use katana_executor::{ExecutionResult, ExecutorFactory, ResultAndStates};
use katana_primitives::block::BlockIdOrTag;
use katana_primitives::receipt::Receipt;
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash, TxHash};
use katana_rpc_api::starknet::StarknetTraceApiServer;
use katana_rpc_types::error::starknet::StarknetApiError;
use katana_rpc_types::trace::FunctionInvocation;
use katana_rpc_types::transaction::BroadcastedTx;
use katana_rpc_types::{FeeEstimate, SimulationFlag};
use starknet::core::types::{
    ComputationResources, DataAvailabilityResources, DataResources, DeclareTransactionTrace,
    DeployAccountTransactionTrace, ExecuteInvocation, ExecutionResources, InvokeTransactionTrace,
    L1HandlerTransactionTrace, RevertedInvocation, SimulatedTransaction, TransactionTrace,
    TransactionTraceWithHash,
};

use super::StarknetApi;

#[async_trait]
impl<EF: ExecutorFactory> StarknetTraceApiServer for StarknetApi<EF> {
    async fn trace_transaction(&self, _: TxHash) -> RpcResult<TransactionTrace> {
        Err(Error::Call(CallError::Custom(ErrorObject::owned(
            METHOD_NOT_FOUND_CODE,
            "Unsupported method - starknet_traceTransaction".to_string(),
            None::<()>,
        ))))
    }

    async fn simulate_transactions(
        &self,
        block_id: BlockIdOrTag,
        transactions: Vec<BroadcastedTx>,
        simulation_flags: Vec<SimulationFlag>,
    ) -> RpcResult<Vec<SimulatedTransaction>> {
        self.on_cpu_blocking_task(move |this| {
            let chain_id = this.inner.sequencer.backend().chain_id;

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
                || this.inner.sequencer.backend().config.disable_validate);
            // If the node is run with fee charge disabled, then we should disable charing fees even
            // if the `SKIP_FEE_CHARGE` flag is not set.
            let should_skip_fee = !(simulation_flags.contains(&SimulationFlag::SkipFeeCharge)
                || this.inner.sequencer.backend().config.disable_fee);

            let flags = katana_executor::SimulationFlag {
                skip_validate: !should_validate,
                skip_fee_transfer: !should_skip_fee,
                ..Default::default()
            };

            let sequencer = &this.inner.sequencer;
            // get the state and block env at the specified block for execution
            let state = this.state(&block_id)?;
            let env = this.block_env_at(&block_id)?;

            // create the executor
            let executor =
                sequencer.backend().executor_factory.with_state_and_block_env(state, env);
            let results = executor.simulate(executables, flags);

            let mut simulated = Vec::with_capacity(results.len());
            for (i, ResultAndStates { result, .. }) in results.into_iter().enumerate() {
                match result {
                    ExecutionResult::Success { trace, receipt } => {
                        let fee_transfer_invocation =
                            trace.fee_transfer_call_info.map(|f| FunctionInvocation::from(f).0);
                        let validate_invocation =
                            trace.validate_call_info.map(|f| FunctionInvocation::from(f).0);
                        let execute_invocation =
                            trace.execute_call_info.map(|f| FunctionInvocation::from(f).0);
                        let revert_reason = trace.revert_error;
                        // TODO: compute the state diff
                        let state_diff = None;

                        let execution_resources = ExecutionResources {
                            computation_resources: ComputationResources {
                                steps: 0,
                                memory_holes: None,
                                segment_arena_builtin: None,
                                ecdsa_builtin_applications: None,
                                ec_op_builtin_applications: None,
                                keccak_builtin_applications: None,
                                bitwise_builtin_applications: None,
                                pedersen_builtin_applications: None,
                                poseidon_builtin_applications: None,
                                range_check_builtin_applications: None,
                            },
                            data_resources: DataResources {
                                data_availability: DataAvailabilityResources {
                                    l1_gas: 0,
                                    l1_data_gas: 0,
                                },
                            },
                        };

                        let transaction_trace = match receipt {
                            Receipt::Invoke(_) => {
                                TransactionTrace::Invoke(InvokeTransactionTrace {
                                    fee_transfer_invocation,
                                    validate_invocation,
                                    state_diff,
                                    execute_invocation: if let Some(revert_reason) = revert_reason {
                                        ExecuteInvocation::Reverted(RevertedInvocation {
                                            revert_reason,
                                        })
                                    } else {
                                        ExecuteInvocation::Success(
                                            execute_invocation
                                                .expect("should exist if not reverted"),
                                        )
                                    },
                                    execution_resources: execution_resources.clone(),
                                })
                            }

                            Receipt::Declare(_) => {
                                TransactionTrace::Declare(DeclareTransactionTrace {
                                    fee_transfer_invocation,
                                    validate_invocation,
                                    state_diff,
                                    execution_resources: execution_resources.clone(),
                                })
                            }

                            Receipt::DeployAccount(_) => {
                                TransactionTrace::DeployAccount(DeployAccountTransactionTrace {
                                    fee_transfer_invocation,
                                    validate_invocation,
                                    state_diff,
                                    constructor_invocation: execute_invocation
                                        .expect("should exist bcs tx succeed"),
                                    execution_resources: execution_resources.clone(),
                                })
                            }

                            Receipt::L1Handler(_) => {
                                TransactionTrace::L1Handler(L1HandlerTransactionTrace {
                                    state_diff,
                                    function_invocation: execute_invocation
                                        .expect("should exist bcs tx succeed"),
                                    execution_resources,
                                })
                            }
                        };

                        let fee = receipt.fee();
                        simulated.push(SimulatedTransaction {
                            transaction_trace,
                            fee_estimation: FeeEstimate {
                                unit: fee.unit,
                                gas_price: fee.gas_price.into(),
                                overall_fee: fee.overall_fee.into(),
                                gas_consumed: fee.gas_consumed.into(),
                                data_gas_price: Default::default(),
                                data_gas_consumed: Default::default(),
                            },
                        })
                    }

                    ExecutionResult::Failed { error } => {
                        return Err(Error::from(StarknetApiError::TransactionExecutionError {
                            transaction_index: i,
                            execution_error: error.to_string(),
                        }));
                    }
                }
            }

            Ok(simulated)
        })
        .await
    }

    async fn trace_block_transactions(
        &self,
        _: BlockIdOrTag,
    ) -> RpcResult<Vec<TransactionTraceWithHash>> {
        Err(Error::Call(CallError::Custom(ErrorObject::owned(
            METHOD_NOT_FOUND_CODE,
            "Unsupported method - starknet_traceBlockTransactions".to_string(),
            None::<()>,
        ))))
    }
}

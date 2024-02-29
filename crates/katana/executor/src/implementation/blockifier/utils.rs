use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use blockifier::block_context::{BlockContext, BlockInfo, ChainInfo, FeeTokenAddresses, GasPrices};
use blockifier::execution::call_info::CallInfo;
use blockifier::execution::common_hints::ExecutionMode;
use blockifier::execution::entry_point::{
    CallEntryPoint, EntryPointExecutionContext, ExecutionResources,
};
use blockifier::execution::errors::EntryPointExecutionError;
use blockifier::fee::fee_utils::calculate_tx_l1_gas_usages;
use blockifier::state::cached_state::{self};
use blockifier::state::state_api::{State, StateReader};
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::errors::{TransactionExecutionError, TransactionFeeError};
use blockifier::transaction::objects::{
    AccountTransactionContext, DeprecatedAccountTransactionContext,
};
use blockifier::transaction::transaction_execution::Transaction;
use blockifier::transaction::transactions::{
    DeclareTransaction, DeployAccountTransaction, ExecutableTransaction, InvokeTransaction,
    L1HandlerTransaction,
};
use katana_primitives::conversion::blockifier::to_class;
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};
use katana_primitives::transaction::{
    DeclareTx, DeployAccountTx, ExecutableTx, ExecutableTxWithHash, InvokeTx,
};
use katana_provider::traits::contract::ContractClassProvider;
use starknet_api::block::{BlockNumber, BlockTimestamp};
use starknet_api::core::{self, ClassHash, CompiledClassHash, Nonce};
use starknet_api::data_availability::DataAvailabilityMode;
use starknet_api::transaction::{
    AccountDeploymentData, Calldata, ContractAddressSalt,
    DeclareTransaction as ApiDeclareTransaction, DeclareTransactionV0V1, DeclareTransactionV2,
    DeclareTransactionV3, DeployAccountTransaction as ApiDeployAccountTransaction,
    DeployAccountTransactionV1, DeployAccountTransactionV3, Fee,
    InvokeTransaction as ApiInvokeTransaction, PaymasterData, Resource, ResourceBounds,
    ResourceBoundsMapping, Tip, TransactionHash, TransactionSignature, TransactionVersion,
};

use super::output::TransactionExecutionInfo;
use super::state::{CachedState, StateDb};
use crate::abstraction::{EntryPointCall, SimulationFlag};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to execute call: {0}")]
    CallError(#[from] EntryPointExecutionError),

    #[error("fee error: {0}")]
    TransactionFee(#[from] TransactionFeeError),

    #[error("failed to execute transaction: {0}")]
    TransactionExecution(#[from] TransactionExecutionError),
}

pub(super) fn transact<S: StateReader>(
    tx: ExecutableTxWithHash,
    state: &mut cached_state::CachedState<S>,
    block_context: &BlockContext,
    simulation_flags: &SimulationFlag,
) -> Result<TransactionExecutionInfo, Error> {
    let validate = !simulation_flags.skip_validate;
    let charge_fee = !simulation_flags.skip_fee_transfer;

    let res = match to_executor_tx(tx) {
        Transaction::AccountTransaction(tx) => {
            tx.execute(state, block_context, charge_fee, validate)
        }
        Transaction::L1HandlerTransaction(tx) => {
            tx.execute(state, block_context, charge_fee, validate)
        }
    }?;

    let gas_used = calculate_tx_l1_gas_usages(&res.actual_resources, block_context)?.gas_usage;
    Ok(TransactionExecutionInfo { inner: res, gas_used })
}

/// Perform a function call on a contract and retrieve the return values.
pub(super) fn call<S: StateReader>(
    request: EntryPointCall,
    state: &mut cached_state::CachedState<S>,
    block_context: &BlockContext,
    initial_gas: u128,
) -> Result<CallInfo, Error> {
    // let inner = &mut state.0.write().inner;
    // let inner = MutRefState::new(inner);

    // let mut state = cached_state::CachedState::new(inner, GlobalContractCache::default());

    let call = CallEntryPoint {
        initial_gas: initial_gas as u64,
        storage_address: request.contract_address.into(),
        entry_point_selector: core::EntryPointSelector(request.entry_point_selector.into()),
        calldata: Calldata(Arc::new(request.calldata.into_iter().map(|f| f.into()).collect())),
        ..Default::default()
    };

    // TODO: this must be false if fees are disabled I assume.
    let limit_steps_by_resources = true;

    // Now, the max step is not given directly to this function.
    // It's computed by a new function max_steps, and it tooks the values
    // from teh block context itself instead of the input give.
    // https://github.com/starkware-libs/blockifier/blob/51b343fe38139a309a69b2482f4b484e8caa5edf/crates/blockifier/src/execution/entry_point.rs#L165
    // The blockifier patch must be adjusted to modify this function to return
    // the limit we have into the block context without min applied:
    // https://github.com/starkware-libs/blockifier/blob/51b343fe38139a309a69b2482f4b484e8caa5edf/crates/blockifier/src/execution/entry_point.rs#L215
    let res = call.execute(
        state,
        &mut ExecutionResources::default(),
        &mut EntryPointExecutionContext::new(
            block_context,
            // TODO: the current does not have Default, let's use the old one for now.
            &AccountTransactionContext::Deprecated(DeprecatedAccountTransactionContext::default()),
            ExecutionMode::Execute,
            limit_steps_by_resources,
        )?,
    )?;

    Ok(res)
}

fn to_executor_tx(tx: ExecutableTxWithHash) -> Transaction {
    let hash = tx.hash;

    match tx.transaction {
        ExecutableTx::Invoke(tx) => match tx {
            InvokeTx::V1(tx) => {
                let calldata = tx.calldata.into_iter().map(|f| f.into()).collect();
                let signature = tx.signature.into_iter().map(|f| f.into()).collect();

                Transaction::AccountTransaction(AccountTransaction::Invoke(InvokeTransaction {
                    tx: ApiInvokeTransaction::V1(starknet_api::transaction::InvokeTransactionV1 {
                        max_fee: Fee(tx.max_fee),
                        nonce: Nonce(tx.nonce.into()),
                        sender_address: tx.sender_address.into(),
                        signature: TransactionSignature(signature),
                        calldata: Calldata(Arc::new(calldata)),
                    }),
                    tx_hash: TransactionHash(hash.into()),
                    only_query: false,
                }))
            }

            InvokeTx::V3(tx) => {
                let calldata = tx.calldata.into_iter().map(|f| f.into()).collect();
                let signature = tx.signature.into_iter().map(|f| f.into()).collect();

                let paymaster_data = tx.paymaster_data.into_iter().map(|f| f.into()).collect();
                let account_deploy_data =
                    tx.account_deployment_data.into_iter().map(|f| f.into()).collect();
                let fee_data_availability_mode = to_api_da_mode(tx.fee_data_availability_mode);
                let nonce_data_availability_mode = to_api_da_mode(tx.nonce_data_availability_mode);

                Transaction::AccountTransaction(AccountTransaction::Invoke(InvokeTransaction {
                    tx: ApiInvokeTransaction::V3(starknet_api::transaction::InvokeTransactionV3 {
                        tip: Tip(tx.tip),
                        nonce: Nonce(tx.nonce.into()),
                        sender_address: tx.sender_address.into(),
                        signature: TransactionSignature(signature),
                        calldata: Calldata(Arc::new(calldata)),
                        paymaster_data: PaymasterData(paymaster_data),
                        account_deployment_data: AccountDeploymentData(account_deploy_data),
                        fee_data_availability_mode,
                        nonce_data_availability_mode,
                        resource_bounds: to_api_resource_bounds(tx.resource_bounds),
                    }),
                    tx_hash: TransactionHash(hash.into()),
                    only_query: false,
                }))
            }
        },

        ExecutableTx::DeployAccount(tx) => match tx {
            DeployAccountTx::V1(tx) => {
                let calldata = tx.constructor_calldata.into_iter().map(|f| f.into()).collect();
                let signature = tx.signature.into_iter().map(|f| f.into()).collect();
                let salt = ContractAddressSalt(tx.contract_address_salt.into());

                Transaction::AccountTransaction(AccountTransaction::DeployAccount(
                    DeployAccountTransaction {
                        contract_address: tx.contract_address.into(),
                        tx: ApiDeployAccountTransaction::V1(DeployAccountTransactionV1 {
                            max_fee: Fee(tx.max_fee),
                            nonce: Nonce(tx.nonce.into()),
                            signature: TransactionSignature(signature),
                            class_hash: ClassHash(tx.class_hash.into()),
                            constructor_calldata: Calldata(Arc::new(calldata)),
                            contract_address_salt: salt,
                        }),
                        tx_hash: TransactionHash(hash.into()),
                        only_query: false,
                    },
                ))
            }

            DeployAccountTx::V3(tx) => {
                let calldata = tx.constructor_calldata.into_iter().map(|f| f.into()).collect();
                let signature = tx.signature.into_iter().map(|f| f.into()).collect();
                let salt = ContractAddressSalt(tx.contract_address_salt.into());

                let paymaster_data = tx.paymaster_data.into_iter().map(|f| f.into()).collect();
                let fee_data_availability_mode = to_api_da_mode(tx.fee_data_availability_mode);
                let nonce_data_availability_mode = to_api_da_mode(tx.nonce_data_availability_mode);

                Transaction::AccountTransaction(AccountTransaction::DeployAccount(
                    DeployAccountTransaction {
                        contract_address: tx.contract_address.into(),
                        tx: ApiDeployAccountTransaction::V3(DeployAccountTransactionV3 {
                            tip: Tip(tx.tip),
                            nonce: Nonce(tx.nonce.into()),
                            signature: TransactionSignature(signature),
                            class_hash: ClassHash(tx.class_hash.into()),
                            constructor_calldata: Calldata(Arc::new(calldata)),
                            contract_address_salt: salt,
                            paymaster_data: PaymasterData(paymaster_data),
                            fee_data_availability_mode,
                            nonce_data_availability_mode,
                            resource_bounds: to_api_resource_bounds(tx.resource_bounds),
                        }),
                        tx_hash: TransactionHash(hash.into()),
                        only_query: false,
                    },
                ))
            }
        },

        ExecutableTx::Declare(tx) => {
            let contract_class = tx.compiled_class;

            let tx = match tx.transaction {
                DeclareTx::V1(tx) => {
                    let signature = tx.signature.into_iter().map(|f| f.into()).collect();

                    ApiDeclareTransaction::V1(DeclareTransactionV0V1 {
                        max_fee: Fee(tx.max_fee),
                        nonce: Nonce(tx.nonce.into()),
                        sender_address: tx.sender_address.into(),
                        signature: TransactionSignature(signature),
                        class_hash: ClassHash(tx.class_hash.into()),
                    })
                }

                DeclareTx::V2(tx) => {
                    let signature = tx.signature.into_iter().map(|f| f.into()).collect();

                    ApiDeclareTransaction::V2(DeclareTransactionV2 {
                        max_fee: Fee(tx.max_fee),
                        nonce: Nonce(tx.nonce.into()),
                        sender_address: tx.sender_address.into(),
                        signature: TransactionSignature(signature),
                        class_hash: ClassHash(tx.class_hash.into()),
                        compiled_class_hash: CompiledClassHash(tx.compiled_class_hash.into()),
                    })
                }

                DeclareTx::V3(tx) => {
                    let signature = tx.signature.into_iter().map(|f| f.into()).collect();

                    let paymaster_data = tx.paymaster_data.into_iter().map(|f| f.into()).collect();
                    let fee_data_availability_mode = to_api_da_mode(tx.fee_data_availability_mode);
                    let nonce_data_availability_mode =
                        to_api_da_mode(tx.nonce_data_availability_mode);
                    let account_deploy_data =
                        tx.account_deployment_data.into_iter().map(|f| f.into()).collect();

                    ApiDeclareTransaction::V3(DeclareTransactionV3 {
                        tip: Tip(tx.tip),
                        nonce: Nonce(tx.nonce.into()),
                        sender_address: tx.sender_address.into(),
                        signature: TransactionSignature(signature),
                        class_hash: ClassHash(tx.class_hash.into()),
                        account_deployment_data: AccountDeploymentData(account_deploy_data),
                        compiled_class_hash: CompiledClassHash(tx.compiled_class_hash.into()),
                        paymaster_data: PaymasterData(paymaster_data),
                        fee_data_availability_mode,
                        nonce_data_availability_mode,
                        resource_bounds: to_api_resource_bounds(tx.resource_bounds),
                    })
                }
            };

            let hash = TransactionHash(hash.into());
            let class = to_class(contract_class).unwrap();
            let tx = DeclareTransaction::new(tx, hash, class).expect("class mismatch");
            Transaction::AccountTransaction(AccountTransaction::Declare(tx))
        }

        ExecutableTx::L1Handler(tx) => {
            let calldata = tx.calldata.into_iter().map(|f| f.into()).collect();
            Transaction::L1HandlerTransaction(L1HandlerTransaction {
                paid_fee_on_l1: Fee(tx.paid_fee_on_l1),
                tx: starknet_api::transaction::L1HandlerTransaction {
                    nonce: core::Nonce(tx.nonce.into()),
                    calldata: Calldata(Arc::new(calldata)),
                    version: TransactionVersion(1u128.into()),
                    contract_address: tx.contract_address.into(),
                    entry_point_selector: core::EntryPointSelector(tx.entry_point_selector.into()),
                },
                tx_hash: TransactionHash(hash.into()),
            })
        }
    }
}

/// Create a block context from the chain environment values.
pub(super) fn block_context_from_envs(block_env: &BlockEnv, cfg_env: &CfgEnv) -> BlockContext {
    let fee_token_addresses = FeeTokenAddresses {
        eth_fee_token_address: cfg_env.fee_token_addresses.eth.into(),
        strk_fee_token_address: cfg_env.fee_token_addresses.strk.into(),
    };

    let gas_prices = GasPrices {
        eth_l1_gas_price: block_env.l1_gas_prices.eth,
        strk_l1_gas_price: block_env.l1_gas_prices.strk,
        eth_l1_data_gas_price: 0,
        strk_l1_data_gas_price: 0,
    };

    BlockContext {
        block_info: BlockInfo {
            gas_prices,
            use_kzg_da: false,
            block_number: BlockNumber(block_env.number),
            block_timestamp: BlockTimestamp(block_env.timestamp),
            sequencer_address: block_env.sequencer_address.into(),
            max_recursion_depth: cfg_env.max_recursion_depth,
            validate_max_n_steps: cfg_env.validate_max_n_steps,
            invoke_tx_max_n_steps: cfg_env.invoke_tx_max_n_steps,
            vm_resource_fee_cost: cfg_env.vm_resource_fee_cost.clone().into(),
        },
        chain_info: ChainInfo { fee_token_addresses, chain_id: cfg_env.chain_id.into() },
    }
}

pub(super) fn state_update_from_cached_state<S: StateDb>(
    state: &CachedState<S>,
) -> StateUpdatesWithDeclaredClasses {
    use katana_primitives::class::{CompiledClass, FlattenedSierraClass};

    let state_diff = state.0.write().inner.to_state_diff();

    let mut declared_compiled_classes: HashMap<katana_primitives::class::ClassHash, CompiledClass> =
        HashMap::new();
    let mut declared_sierra_classes: HashMap<
        katana_primitives::class::ClassHash,
        FlattenedSierraClass,
    > = HashMap::new();

    for (class_hash, _) in &state_diff.class_hash_to_compiled_class_hash {
        let hash = class_hash.0.into();
        let class = state.class(hash).unwrap().expect("must exist if declared");

        if let CompiledClass::Class(_) = class {
            let sierra = state.sierra_class(hash).unwrap().expect("must exist if declared");
            declared_sierra_classes.insert(hash, sierra);
        }

        declared_compiled_classes.insert(hash, class);
    }

    let nonce_updates =
        state_diff
            .address_to_nonce
            .into_iter()
            .map(|(key, value)| (key.into(), value.0.into()))
            .collect::<HashMap<
                katana_primitives::contract::ContractAddress,
                katana_primitives::contract::Nonce,
            >>();

    let storage_updates = state_diff
        .storage_updates
        .into_iter()
        .map(|(addr, entries)| {
            let entries = entries
                .into_iter()
                .map(|(k, v)| ((*k.0.key()).into(), v.into()))
                .collect::<HashMap<
                    katana_primitives::contract::StorageKey,
                    katana_primitives::contract::StorageValue,
                >>();

            (addr.into(), entries)
        })
        .collect::<HashMap<katana_primitives::contract::ContractAddress, _>>();

    let contract_updates =
        state_diff
            .address_to_class_hash
            .into_iter()
            .map(|(key, value)| (key.into(), value.0.into()))
            .collect::<HashMap<
                katana_primitives::contract::ContractAddress,
                katana_primitives::class::ClassHash,
            >>();

    let declared_classes =
        state_diff
            .class_hash_to_compiled_class_hash
            .into_iter()
            .map(|(key, value)| (key.0.into(), value.0.into()))
            .collect::<HashMap<
                katana_primitives::class::ClassHash,
                katana_primitives::class::CompiledClassHash,
            >>();

    StateUpdatesWithDeclaredClasses {
        declared_sierra_classes,
        declared_compiled_classes,
        state_updates: StateUpdates {
            nonce_updates,
            storage_updates,
            contract_updates,
            declared_classes,
        },
    }
}

fn to_api_da_mode(mode: starknet::core::types::DataAvailabilityMode) -> DataAvailabilityMode {
    match mode {
        starknet::core::types::DataAvailabilityMode::L1 => DataAvailabilityMode::L1,
        starknet::core::types::DataAvailabilityMode::L2 => DataAvailabilityMode::L2,
    }
}

fn to_api_resource_bounds(
    resource_bounds: starknet::core::types::ResourceBoundsMapping,
) -> ResourceBoundsMapping {
    let l1_gas = ResourceBounds {
        max_amount: resource_bounds.l1_gas.max_amount,
        max_price_per_unit: resource_bounds.l1_gas.max_price_per_unit,
    };

    let l2_gas = ResourceBounds {
        max_amount: resource_bounds.l2_gas.max_amount,
        max_price_per_unit: resource_bounds.l2_gas.max_price_per_unit,
    };

    ResourceBoundsMapping(BTreeMap::from([(Resource::L1Gas, l1_gas), (Resource::L2Gas, l2_gas)]))
}

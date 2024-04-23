use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use blockifier::block::{BlockInfo, GasPrices};
use blockifier::context::{BlockContext, ChainInfo, FeeTokenAddresses, TransactionContext};
use blockifier::execution::call_info::{
    CallExecution, CallInfo, OrderedEvent, OrderedL2ToL1Message,
};
use blockifier::execution::common_hints::ExecutionMode;
use blockifier::execution::contract_class::{ContractClass, ContractClassV0, ContractClassV1};
use blockifier::execution::entry_point::{CallEntryPoint, CallType, EntryPointExecutionContext};
use blockifier::fee::fee_utils::{calculate_tx_fee, calculate_tx_gas_vector};
use blockifier::state::cached_state::{self};
use blockifier::state::state_api::{State, StateReader};
use blockifier::transaction::account_transaction::AccountTransaction;
use blockifier::transaction::objects::{
    DeprecatedTransactionInfo, FeeType, HasRelatedFeeType, TransactionExecutionInfo, TransactionInfo,
};
use blockifier::transaction::transaction_execution::Transaction;
use blockifier::transaction::transactions::{
    DeclareTransaction, DeployAccountTransaction, ExecutableTransaction, InvokeTransaction,
    L1HandlerTransaction,
};
use blockifier::versioned_constants::VersionedConstants;
use cairo_vm::types::errors::program_errors::ProgramError;
use cairo_vm::vm::runners::cairo_runner::ExecutionResources;
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::fee::TxFeeInfo;
use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};
use katana_primitives::trace::TxExecInfo;
use katana_primitives::transaction::{
    DeclareTx, DeployAccountTx, ExecutableTx, ExecutableTxWithHash, InvokeTx,
};
use katana_primitives::{event, message, trace, FieldElement};
use katana_provider::traits::contract::ContractClassProvider;
use starknet::core::types::PriceUnit;
use starknet::core::utils::parse_cairo_short_string;
use starknet_api::block::{BlockNumber, BlockTimestamp};
use starknet_api::core::{
    self, ChainId, ClassHash, CompiledClassHash, ContractAddress, Nonce, PatriciaKey,
};
use starknet_api::data_availability::DataAvailabilityMode;
use starknet_api::deprecated_contract_class::EntryPointType;
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;
use starknet_api::transaction::{
    AccountDeploymentData, Calldata, ContractAddressSalt,
    DeclareTransaction as ApiDeclareTransaction, DeclareTransactionV0V1, DeclareTransactionV2,
    DeclareTransactionV3, DeployAccountTransaction as ApiDeployAccountTransaction,
    DeployAccountTransactionV1, DeployAccountTransactionV3, Fee,
    InvokeTransaction as ApiInvokeTransaction, PaymasterData, Resource, ResourceBounds,
    ResourceBoundsMapping, Tip, TransactionHash, TransactionSignature, TransactionVersion,
};

use super::state::{CachedState, StateDb};
use crate::abstraction::{EntryPointCall, SimulationFlag};
use crate::ExecutionError;

pub(super) fn transact<S: StateReader>(
    tx: ExecutableTxWithHash,
    state: &mut cached_state::CachedState<S>,
    block_context: &BlockContext,
    simulation_flags: &SimulationFlag,
) -> Result<(TransactionExecutionInfo, TxFeeInfo), ExecutionError> {
    let validate = !simulation_flags.skip_validate;
    let charge_fee = !simulation_flags.skip_fee_transfer;

    let transaction = to_executor_tx(tx);
    let fee_type = get_fee_type_from_tx(&transaction);

    let info = match transaction {
        Transaction::AccountTransaction(tx) => {
            tx.execute(state, block_context, charge_fee, validate)
        }
        Transaction::L1HandlerTransaction(tx) => {
            tx.execute(state, block_context, charge_fee, validate)
        }
    }?;

    // There are a few case where the `actual_fee` field of the transaction info is not set where
    // the fee is skipped and thus not charged for the transaction (e.g. when the
    // `skip_fee_transfer` is explicitly set, or when the transaction `max_fee` is set to 0). In
    // these cases, we still want to calculate the fee.
    let overall_fee = if info.actual_fee == Fee(0) {
        calculate_tx_fee(&info.actual_resources, block_context, &fee_type)?.0
    } else {
        info.actual_fee.0
    };

    let gas_consumed =
        calculate_tx_gas_vector(&info.actual_resources, block_context.versioned_constants())?
            .l1_gas;

    let (unit, gas_price) = match fee_type {
        FeeType::Eth => (PriceUnit::Wei, block_context.block_info().gas_prices.eth_l1_gas_price),
        FeeType::Strk => (PriceUnit::Fri, block_context.block_info().gas_prices.strk_l1_gas_price),
    };

    let fee = TxFeeInfo { gas_consumed, gas_price, unit, overall_fee };
    Ok((info, fee))
}

/// Perform a function call on a contract and retrieve the return values.
pub fn call<S: StateReader>(
    request: EntryPointCall,
    state: S,
    block_context: &BlockContext,
    initial_gas: u128,
) -> Result<Vec<FieldElement>, ExecutionError> {
    let mut state = cached_state::CachedState::new(state, Default::default());

    let call = CallEntryPoint {
        initial_gas: initial_gas as u64,
        storage_address: to_blk_address(request.contract_address),
        entry_point_selector: core::EntryPointSelector(request.entry_point_selector.into()),
        calldata: Calldata(Arc::new(request.calldata.into_iter().map(|f| f.into()).collect())),
        ..Default::default()
    };

    // TODO: this must be false if fees are disabled I assume.
    let limit_steps_by_resources = true;

    // Now, the max step is not given directly to this function.
    // It's computed by a new function max_steps, and it tooks the values
    // from the block context itself instead of the input give. The dojoengine
    // fork of the blockifier ensures we're not limited by the min function applied
    // by starkware.
    // https://github.com/starkware-libs/blockifier/blob/4fd71645b45fd1deb6b8e44802414774ec2a2ec1/crates/blockifier/src/execution/entry_point.rs#L159
    // https://github.com/dojoengine/blockifier/blob/5f58be8961ddf84022dd739a8ab254e32c435075/crates/blockifier/src/execution/entry_point.rs#L188

    let res = call.execute(
        &mut state,
        &mut ExecutionResources::default(),
        &mut EntryPointExecutionContext::new(
            TransactionContext {
                block_context,
                tx_info: TransactionInfo::Deprecated(DeprecatedTransactionInfo::default())
            },
            ExecutionMode::Execute,
            limit_steps_by_resources,
        )
        .expect("shouldn't fail"),
    )?;

    let retdata = res.execution.retdata.0;
    let retdata = retdata.into_iter().map(|f| f.into()).collect::<Vec<FieldElement>>();
    Ok(retdata)
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
                        sender_address: to_blk_address(tx.sender_address),
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
                        sender_address: to_blk_address(tx.sender_address),
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
                        contract_address: to_blk_address(tx.contract_address),
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
                        contract_address: to_blk_address(tx.contract_address),
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
                        sender_address: to_blk_address(tx.sender_address),
                        signature: TransactionSignature(signature),
                        class_hash: ClassHash(tx.class_hash.into()),
                    })
                }

                DeclareTx::V2(tx) => {
                    let signature = tx.signature.into_iter().map(|f| f.into()).collect();

                    ApiDeclareTransaction::V2(DeclareTransactionV2 {
                        max_fee: Fee(tx.max_fee),
                        nonce: Nonce(tx.nonce.into()),
                        sender_address: to_blk_address(tx.sender_address),
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
                        sender_address: to_blk_address(tx.sender_address),
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
                    contract_address: to_blk_address(tx.contract_address),
                    entry_point_selector: core::EntryPointSelector(tx.entry_point_selector.into()),
                },
                tx_hash: TransactionHash(hash.into()),
            })
        }
    }
}

/// Create a block context from the chain environment values.
pub(crate) fn block_context_from_envs(block_env: &BlockEnv, cfg_env: &CfgEnv) -> BlockContext {
    let fee_token_addresses = FeeTokenAddresses {
        eth_fee_token_address: to_blk_address(cfg_env.fee_token_addresses.eth),
        strk_fee_token_address: to_blk_address(cfg_env.fee_token_addresses.strk),
    };

    let gas_prices = GasPrices {
        eth_l1_gas_price: block_env.l1_gas_prices.eth,
        strk_l1_gas_price: block_env.l1_gas_prices.strk,
        eth_l1_data_gas_price: 0,
        strk_l1_data_gas_price: 0,
    };

    let block_info = BlockInfo {
        block_number: BlockNumber(block_env.number),
        block_timestamp: BlockTimestamp(block_env.timestamp),
        sequencer_address: to_blk_address(block_env.sequencer_address),
        gas_prices,
        use_kzg_da: false,
    };

    let chain_info = ChainInfo { fee_token_addresses, chain_id: to_blk_chain_id(cfg_env.chain_id) };

    let versioned_constants = VersionedConstants::new(
        cfg_env.max_recursion_depth,
        cfg_env.validate_max_n_steps,
        cfg_env.invoke_tx_max_n_steps,
        cfg_env.vm_resource_fee_cost.clone(),
    );

    BlockContext::new_unchecked(&block_info, &chain_info, &versioned_constants)
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
            .map(|(key, value)| (to_address(key), value.0.into()))
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

            (to_address(addr), entries)
        })
        .collect::<HashMap<katana_primitives::contract::ContractAddress, _>>();

    let contract_updates =
        state_diff
            .address_to_class_hash
            .into_iter()
            .map(|(key, value)| (to_address(key), value.0.into()))
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

/// Get the fee type of a transaction. The fee type determines the token used to pay for the
/// transaction.
fn get_fee_type_from_tx(transaction: &Transaction) -> FeeType {
    match transaction {
        Transaction::AccountTransaction(tx) => tx.fee_type(),
        Transaction::L1HandlerTransaction(tx) => tx.fee_type(),
    }
}

pub fn to_blk_address(address: katana_primitives::contract::ContractAddress) -> ContractAddress {
    ContractAddress(patricia_key!(address.0))
}

pub fn to_address(address: ContractAddress) -> katana_primitives::contract::ContractAddress {
    katana_primitives::contract::ContractAddress((*address.0.key()).into())
}

pub fn to_blk_chain_id(chain_id: katana_primitives::chain::ChainId) -> ChainId {
    match chain_id {
        katana_primitives::chain::ChainId::Named(named) => ChainId(named.name().to_string()),
        katana_primitives::chain::ChainId::Id(id) => {
            let id = parse_cairo_short_string(&id).expect("valid cairo string");
            ChainId(id)
        }
    }
}

pub fn to_class(
    class: katana_primitives::class::CompiledClass,
) -> Result<ContractClass, ProgramError> {
    match class {
        katana_primitives::class::CompiledClass::Deprecated(class) => {
            Ok(ContractClass::V0(ContractClassV0::try_from(class)?))
        }
        katana_primitives::class::CompiledClass::Class(class) => {
            Ok(ContractClass::V1(ContractClassV1::try_from(class.casm)?))
        }
    }
}

/// TODO: remove this function once starknet api 0.8.0 is supported.
fn starknet_api_ethaddr_to_felt(value: starknet_api::core::EthAddress) -> FieldElement {
    let mut bytes = [0u8; 32];
    // Padding H160 with zeros to 32 bytes (big endian)
    bytes[12..32].copy_from_slice(value.0.as_bytes());
    let stark_felt = starknet_api::hash::StarkFelt::new(bytes).expect("valid slice for stark felt");
    stark_felt.into()
}

pub fn to_exec_info(exec_info: TransactionExecutionInfo) -> TxExecInfo {
    TxExecInfo {
        validate_call_info: exec_info.validate_call_info.map(to_call_info),
        execute_call_info: exec_info.execute_call_info.map(to_call_info),
        fee_transfer_call_info: exec_info.fee_transfer_call_info.map(to_call_info),
        actual_fee: exec_info.actual_fee.0,
        actual_resources: exec_info
            .actual_resources
            .0
            .into_iter()
            .map(|(k, v)| (k, v as u64))
            .collect(),
        revert_error: exec_info.revert_error.clone(),
    }
}

fn to_call_info(call: CallInfo) -> trace::CallInfo {
    let contract_address = to_address(call.call.storage_address);
    let caller_address = to_address(call.call.caller_address);
    let code_address = call.call.code_address.map(to_address);
    let class_hash = call.call.class_hash.map(|a| a.0.into());
    let entry_point_selector = call.call.entry_point_selector.0.into();
    let calldata = call.call.calldata.0.iter().map(|f| (*f).into()).collect();
    let retdata = call.execution.retdata.0.into_iter().map(|f| f.into()).collect();

    let builtin_counter = call.vm_resources.builtin_instance_counter;
    let execution_resources = trace::ExecutionResources {
        n_steps: call.vm_resources.n_steps as u64,
        n_memory_holes: call.vm_resources.n_memory_holes as u64,
        builtin_instance_counter: builtin_counter.into_iter().map(|(k, v)| (k, v as u64)).collect(),
    };

    let CallExecution { events, l2_to_l1_messages, .. } = call.execution;

    let events = events.into_iter().map(to_ordered_event).collect();
    let l1_msg =
        l2_to_l1_messages.into_iter().map(|m| to_l2_l1_messages(m, contract_address)).collect();

    let call_type = match call.call.call_type {
        CallType::Call => trace::CallType::Call,
        CallType::Delegate => trace::CallType::Delegate,
    };

    let entry_point_type = match call.call.entry_point_type {
        EntryPointType::External => trace::EntryPointType::External,
        EntryPointType::L1Handler => trace::EntryPointType::L1Handler,
        EntryPointType::Constructor => trace::EntryPointType::Constructor,
    };

    let storage_read_values = call.storage_read_values.into_iter().map(|f| f.into()).collect();
    let storg_keys = call.accessed_storage_keys.into_iter().map(|k| (*k.0.key()).into()).collect();
    let inner_calls = call.inner_calls.into_iter().map(to_call_info).collect();

    trace::CallInfo {
        contract_address,
        caller_address,
        call_type,
        code_address,
        class_hash,
        entry_point_selector,
        entry_point_type,
        calldata,
        retdata,
        execution_resources,
        events,
        l2_to_l1_messages: l1_msg,
        storage_read_values,
        accessed_storage_keys: storg_keys,
        inner_calls,
        gas_consumed: call.execution.gas_consumed as u128,
        failed: call.execution.failed,
    }
}

fn to_ordered_event(e: OrderedEvent) -> event::OrderedEvent {
    event::OrderedEvent {
        order: e.order as u64,
        keys: e.event.keys.into_iter().map(|f| f.0.into()).collect(),
        data: e.event.data.0.into_iter().map(FieldElement::from).collect(),
    }
}

fn to_l2_l1_messages(
    m: OrderedL2ToL1Message,
    from_address: katana_primitives::contract::ContractAddress,
) -> message::OrderedL2ToL1Message {
    let order = m.order as u64;
    let to_address = starknet_api_ethaddr_to_felt(m.message.to_address);
    let payload = m.message.payload.0.into_iter().map(FieldElement::from).collect();
    message::OrderedL2ToL1Message { order, from_address, to_address, payload }
}

#[cfg(test)]
mod tests {

    use std::collections::HashSet;

    use cairo_vm::vm::runners::cairo_runner::ExecutionResources;
    use katana_primitives::chain::{ChainId, NamedChainId};
    use starknet_api::core::EntryPointSelector;
    use starknet_api::hash::StarkFelt;
    use starknet_api::stark_felt;
    use starknet_api::transaction::{EventContent, EventData, EventKey};

    use super::*;

    #[test]
    fn convert_chain_id() {
        let mainnet = to_blk_chain_id(ChainId::Named(NamedChainId::Mainnet));
        let goerli = to_blk_chain_id(ChainId::Named(NamedChainId::Goerli));
        let sepolia = to_blk_chain_id(ChainId::Named(NamedChainId::Sepolia));

        assert_eq!(mainnet.0, parse_cairo_short_string(&NamedChainId::Mainnet.id()).unwrap());
        assert_eq!(goerli.0, parse_cairo_short_string(&NamedChainId::Goerli.id()).unwrap());
        assert_eq!(sepolia.0, parse_cairo_short_string(&NamedChainId::Sepolia.id()).unwrap());
    }

    fn create_blockifier_call_info() -> CallInfo {
        let top_events = vec![OrderedEvent {
            order: 0,
            event: EventContent {
                data: EventData(vec![888u128.into()]),
                keys: vec![EventKey(999u128.into())],
            },
        }];
        let nested_events = vec![
            OrderedEvent {
                order: 1,
                event: EventContent {
                    data: EventData(vec![889u128.into()]),
                    keys: vec![EventKey(990u128.into())],
                },
            },
            OrderedEvent {
                order: 2,
                event: EventContent {
                    data: EventData(vec![0u128.into()]),
                    keys: vec![EventKey(9u128.into())],
                },
            },
        ];

        let nested_call = CallInfo {
            execution: CallExecution { events: nested_events, ..Default::default() },
            ..Default::default()
        };

        CallInfo {
            call: CallEntryPoint {
                class_hash: None,
                initial_gas: 77,
                call_type: CallType::Call,
                caller_address: 200u128.into(),
                storage_address: 100u128.into(),
                code_address: Some(100u128.into()),
                entry_point_type: EntryPointType::External,
                calldata: Calldata(Arc::new(vec![stark_felt!(1_u8)])),
                entry_point_selector: EntryPointSelector(stark_felt!(999_u32)),
            },
            execution: CallExecution {
                failed: true,
                gas_consumed: 12345,
                events: top_events,
                ..Default::default()
            },
            storage_read_values: vec![stark_felt!(1_u8), stark_felt!(2_u8)],
            accessed_storage_keys: HashSet::from([3u128.into(), 4u128.into(), 5u128.into()]),
            resources: ExecutionResources {
                n_steps: 1_000_000,
                n_memory_holes: 9_000,
                builtin_instance_counter: HashMap::from([
                    ("ecdsa_builtin".into(), 50),
                    ("pedersen_builtin".into(), 9),
                ]),
            },
            inner_calls: vec![nested_call],
        }
    }

    #[test]
    fn convert_call_info() {
        // setup expected values
        let call = create_blockifier_call_info();

        let expected_contract_address = to_address(call.call.storage_address);
        let expected_caller_address = to_address(call.call.caller_address);
        let expected_code_address = call.call.code_address.map(to_address);
        let expected_class_hash = call.call.class_hash.map(|a| a.0.into());
        let expected_entry_point_selector = call.call.entry_point_selector.0.into();
        let expected_calldata: Vec<FieldElement> =
            call.call.calldata.0.iter().map(|f| (*f).into()).collect();
        let expected_retdata: Vec<FieldElement> =
            call.execution.retdata.0.iter().map(|f| (*f).into()).collect();

        let builtin_counter = call.vm_resources.builtin_instance_counter.clone();
        let expected_execution_resources = trace::ExecutionResources {
            n_steps: call.vm_resources.n_steps as u64,
            n_memory_holes: call.vm_resources.n_memory_holes as u64,
            builtin_instance_counter: builtin_counter
                .into_iter()
                .map(|(k, v)| (k, v as u64))
                .collect(),
        };

        let CallExecution { events, l2_to_l1_messages, .. } = call.execution.clone();
        let expected_events: Vec<_> = events.into_iter().map(to_ordered_event).collect();
        let expected_l2_to_l1_msg: Vec<_> = l2_to_l1_messages
            .into_iter()
            .map(|m| to_l2_l1_messages(m, expected_contract_address))
            .collect();

        let expected_call_type = match call.call.call_type {
            CallType::Call => trace::CallType::Call,
            CallType::Delegate => trace::CallType::Delegate,
        };

        let expected_entry_point_type = match call.call.entry_point_type {
            EntryPointType::External => trace::EntryPointType::External,
            EntryPointType::L1Handler => trace::EntryPointType::L1Handler,
            EntryPointType::Constructor => trace::EntryPointType::Constructor,
        };

        let expected_storage_read_values: Vec<FieldElement> =
            call.storage_read_values.iter().map(|f| (*f).into()).collect();
        let expected_storage_keys: HashSet<FieldElement> =
            call.accessed_storage_keys.iter().map(|k| (*k.0.key()).into()).collect();
        let expected_inner_calls: Vec<_> =
            call.inner_calls.clone().into_iter().map(to_call_info).collect();

        let expected_gas_consumed = call.execution.gas_consumed as u128;
        let expected_failed = call.execution.failed;

        // convert to call info
        let call = to_call_info(call.clone());

        // assert actual values
        assert_eq!(call.contract_address, expected_contract_address);
        assert_eq!(call.caller_address, expected_caller_address);
        assert_eq!(call.code_address, expected_code_address);
        assert_eq!(call.class_hash, expected_class_hash);
        assert_eq!(call.entry_point_selector, expected_entry_point_selector);
        assert_eq!(call.calldata, expected_calldata);
        assert_eq!(call.retdata, expected_retdata);
        assert_eq!(call.execution_resources, expected_execution_resources);
        assert_eq!(call.events, expected_events);
        assert_eq!(call.l2_to_l1_messages, expected_l2_to_l1_msg);
        assert_eq!(call.call_type, expected_call_type);
        assert_eq!(call.entry_point_type, expected_entry_point_type);
        assert_eq!(call.storage_read_values, expected_storage_read_values);
        assert_eq!(call.accessed_storage_keys, expected_storage_keys);
        assert_eq!(call.inner_calls, expected_inner_calls);
        assert_eq!(call.gas_consumed, expected_gas_consumed);
        assert_eq!(call.failed, expected_failed);
    }
}

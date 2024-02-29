use std::collections::HashMap;
use std::sync::Arc;

use ::blockifier::block_context::BlockContext;
use ::blockifier::execution::call_info::CallInfo;
use ::blockifier::execution::common_hints::ExecutionMode;
use ::blockifier::execution::entry_point::{
    CallEntryPoint, EntryPointExecutionContext, ExecutionResources,
};
use ::blockifier::execution::errors::EntryPointExecutionError;
use ::blockifier::state::cached_state::{CachedState, GlobalContractCache, MutRefState};
use ::blockifier::transaction::objects::AccountTransactionContext;
use blockifier::block_context::{BlockInfo, ChainInfo, FeeTokenAddresses, GasPrices};
use blockifier::fee::fee_utils::{calculate_l1_gas_by_vm_usage, extract_l1_gas_and_vm_usage};
use blockifier::state::state_api::State;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::{
    DeprecatedAccountTransactionContext, HasRelatedFeeType, ResourcesMapping,
    TransactionExecutionInfo,
};
use cairo_vm::vm::runners::builtin_runner::{
    BITWISE_BUILTIN_NAME, EC_OP_BUILTIN_NAME, HASH_BUILTIN_NAME, KECCAK_BUILTIN_NAME,
    POSEIDON_BUILTIN_NAME, RANGE_CHECK_BUILTIN_NAME, SEGMENT_ARENA_BUILTIN_NAME,
    SIGNATURE_BUILTIN_NAME,
};
use convert_case::{Case, Casing};
use katana_primitives::contract::ContractAddress;
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::receipt::{Event, MessageToL1};
use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};
use katana_primitives::transaction::{ExecutableTx, ExecutableTxWithHash};
use katana_primitives::FieldElement;
use katana_provider::traits::contract::ContractClassProvider;
use katana_provider::traits::state::StateProvider;
use starknet::core::types::{
    DeclareTransactionTrace, DeployAccountTransactionTrace, ExecuteInvocation, FeeEstimate,
    FunctionInvocation, InvokeTransactionTrace, L1HandlerTransactionTrace, PriceUnit,
    RevertedInvocation, SimulatedTransaction, TransactionTrace,
};
use starknet::core::utils::parse_cairo_short_string;
use starknet::macros::felt;
use starknet_api::block::{BlockNumber, BlockTimestamp};
use starknet_api::core::EntryPointSelector;
use starknet_api::hash::StarkFelt;
use starknet_api::transaction::Calldata;
use tracing::trace;

use super::state::{CachedStateWrapper, StateRefDb};
use super::transactions::BlockifierTx;
use super::TransactionExecutor;

#[derive(Debug)]
pub struct EntryPointCall {
    /// The address of the contract whose function you're calling.
    pub contract_address: ContractAddress,
    /// The input to the function.
    pub calldata: Vec<FieldElement>,
    /// The function selector.
    pub entry_point_selector: FieldElement,
}

/// Perform a function call on a contract and retrieve the return values.
pub fn call(
    request: EntryPointCall,
    block_context: BlockContext,
    state: Box<dyn StateProvider>,
) -> Result<Vec<FieldElement>, TransactionExecutionError> {
    let res = raw_call(request, block_context, state, 1_000_000_000)?;
    let retdata = res.execution.retdata.0;
    let retdata = retdata.into_iter().map(|f| f.into()).collect::<Vec<FieldElement>>();
    Ok(retdata)
}

/// Estimate the execution fee for a list of transactions.
pub fn estimate_fee(
    transactions: impl Iterator<Item = ExecutableTxWithHash>,
    block_context: BlockContext,
    state: Box<dyn StateProvider>,
    validate: bool,
) -> Result<Vec<FeeEstimate>, TransactionExecutionError> {
    let state = CachedStateWrapper::new(StateRefDb(state));
    let results = TransactionExecutor::new(&state, &block_context, true, validate, transactions)
        .with_error_log()
        .execute();

    results
        .into_iter()
        .map(|res| {
            let exec_info = res?;

            if exec_info.revert_error.is_some() {
                return Err(TransactionExecutionError::ExecutionError(
                    EntryPointExecutionError::ExecutionFailed { error_data: Default::default() },
                ));
            }

            calculate_execution_fee(&block_context, &exec_info)
        })
        .collect::<Result<Vec<_>, _>>()
}

/// Simulate a transaction's execution on the state
pub fn simulate_transaction(
    transaction: ExecutableTxWithHash,
    block_context: &BlockContext,
    state: Box<dyn StateProvider>,
    validate: bool,
    charge_fee: bool,
) -> Result<SimulatedTransaction, TransactionExecutionError> {
    let state = CachedStateWrapper::new(StateRefDb::from(state));
    let result = TransactionExecutor::new(
        &state,
        block_context,
        charge_fee,
        validate,
        vec![transaction.clone()].into_iter(),
    )
    .with_error_log()
    .next()
    .ok_or(TransactionExecutionError::ExecutionError(
        EntryPointExecutionError::ExecutionFailed { error_data: Default::default() },
    ))?;
    let result = result?;

    let function_invocation =
        result.execute_call_info.as_ref().map(function_invocation_from_call_info).ok_or(
            TransactionExecutionError::ExecutionError(EntryPointExecutionError::ExecutionFailed {
                error_data: Default::default(),
            }),
        );

    let validate_invocation =
        result.validate_call_info.as_ref().map(function_invocation_from_call_info);

    let fee_transfer_invocation =
        result.fee_transfer_call_info.as_ref().map(function_invocation_from_call_info);

    let transaction_trace = match &transaction.transaction {
        ExecutableTx::Declare(_) => TransactionTrace::Declare(DeclareTransactionTrace {
            validate_invocation,
            fee_transfer_invocation,
            state_diff: None,
        }),
        ExecutableTx::DeployAccount(_) => {
            TransactionTrace::DeployAccount(DeployAccountTransactionTrace {
                constructor_invocation: function_invocation?,
                validate_invocation,
                fee_transfer_invocation,
                state_diff: None,
            })
        }
        ExecutableTx::Invoke(_) => TransactionTrace::Invoke(InvokeTransactionTrace {
            validate_invocation,
            execute_invocation: if let Some(revert_reason) = result.revert_error {
                ExecuteInvocation::Reverted(RevertedInvocation { revert_reason })
            } else {
                ExecuteInvocation::Success(function_invocation?)
            },
            fee_transfer_invocation,
            state_diff: None,
        }),
        ExecutableTx::L1Handler(_) => TransactionTrace::L1Handler(L1HandlerTransactionTrace {
            function_invocation: function_invocation?,
            state_diff: None,
        }),
    };

    let execute_gas_consumed =
        result.execute_call_info.map(|e| e.execution.gas_consumed).unwrap_or_default();
    let validate_gas_consumed =
        result.validate_call_info.map(|e| e.execution.gas_consumed).unwrap_or_default();
    let gas_consumed = execute_gas_consumed + validate_gas_consumed;
    let overall_fee = result.actual_fee.0 as u64;
    let gas_price = if gas_consumed != 0 { overall_fee / gas_consumed } else { 0 };

    let blockifier_tx = BlockifierTx::from(transaction);
    let fee_type = match blockifier_tx.0 {
        blockifier::transaction::transaction_execution::Transaction::AccountTransaction(tx) => {
            tx.fee_type()
        }
        blockifier::transaction::transaction_execution::Transaction::L1HandlerTransaction(tx) => {
            tx.fee_type()
        }
    };

    let fee_estimation = FeeEstimate {
        gas_price: FieldElement::from(gas_price),
        gas_consumed: FieldElement::from(execute_gas_consumed + validate_gas_consumed),
        overall_fee: FieldElement::from(overall_fee),
        unit: match fee_type {
            blockifier::transaction::objects::FeeType::Eth => PriceUnit::Wei,
            blockifier::transaction::objects::FeeType::Strk => PriceUnit::Fri,
        },
    };

    Ok(SimulatedTransaction { transaction_trace, fee_estimation })
}

/// Perform a raw entrypoint call of a contract.
pub fn raw_call(
    request: EntryPointCall,
    block_context: BlockContext,
    state: Box<dyn StateProvider>,
    initial_gas: u64,
) -> Result<CallInfo, TransactionExecutionError> {
    let mut state = CachedState::new(StateRefDb(state), GlobalContractCache::default());
    let mut state = CachedState::new(MutRefState::new(&mut state), GlobalContractCache::default());

    let call = CallEntryPoint {
        initial_gas,
        storage_address: request.contract_address.into(),
        entry_point_selector: EntryPointSelector(request.entry_point_selector.into()),
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
    call.execute(
        &mut state,
        &mut ExecutionResources::default(),
        &mut EntryPointExecutionContext::new(
            &block_context,
            // TODO: the current does not have Default, let's use the old one for now.
            &AccountTransactionContext::Deprecated(DeprecatedAccountTransactionContext::default()),
            ExecutionMode::Execute,
            limit_steps_by_resources,
        )?,
    )
    .map_err(TransactionExecutionError::ExecutionError)
}

/// Calculate the fee of a transaction execution.
pub fn calculate_execution_fee(
    block_context: &BlockContext,
    exec_info: &TransactionExecutionInfo,
) -> Result<FeeEstimate, TransactionExecutionError> {
    let (l1_gas_usage, vm_resources) = extract_l1_gas_and_vm_usage(&exec_info.actual_resources);
    let l1_gas_by_vm_usage = calculate_l1_gas_by_vm_usage(block_context, &vm_resources)?;

    let total_l1_gas_usage = l1_gas_usage as f64 + l1_gas_by_vm_usage;

    // Gas prices are now in two currencies: eth and strk.
    // For now let's only consider eth to be compatible with V2.
    // https://github.com/starkware-libs/blockifier/blob/51b343fe38139a309a69b2482f4b484e8caa5edf/crates/blockifier/src/block_context.rs#L19C26-L19C26
    // https://github.com/starkware-libs/blockifier/blob/51b343fe38139a309a69b2482f4b484e8caa5edf/crates/blockifier/src/block_context.rs#L49
    let gas_price = block_context.block_info.gas_prices.eth_l1_gas_price as u64;
    let gas_consumed = total_l1_gas_usage.ceil() as u64;
    let overall_fee = total_l1_gas_usage.ceil() as u64 * gas_price;

    Ok(FeeEstimate {
        gas_price: gas_price.into(),
        gas_consumed: gas_consumed.into(),
        overall_fee: overall_fee.into(),
        unit: PriceUnit::Wei,
    })
}

/// Create a block context from the chain environment values.
pub fn block_context_from_envs(block_env: &BlockEnv, cfg_env: &CfgEnv) -> BlockContext {
    let fee_token_addresses = FeeTokenAddresses {
        eth_fee_token_address: cfg_env.fee_token_addresses.eth.into(),
        strk_fee_token_address: ContractAddress(felt!("0xb00b5")).into(),
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
            block_number: BlockNumber(block_env.number),
            block_timestamp: BlockTimestamp(block_env.timestamp),
            sequencer_address: block_env.sequencer_address.into(),
            vm_resource_fee_cost: cfg_env.vm_resource_fee_cost.clone().into(),
            validate_max_n_steps: cfg_env.validate_max_n_steps,
            invoke_tx_max_n_steps: cfg_env.invoke_tx_max_n_steps,
            max_recursion_depth: cfg_env.max_recursion_depth,
            use_kzg_da: false,
        },
        chain_info: ChainInfo { fee_token_addresses, chain_id: cfg_env.chain_id.into() },
    }
}

pub(crate) fn warn_message_transaction_error_exec_error(err: &TransactionExecutionError) {
    match err {
        TransactionExecutionError::ExecutionError(ref eperr) => match eperr {
            EntryPointExecutionError::ExecutionFailed { error_data } => {
                let mut reasons: Vec<String> = vec![];
                error_data.iter().for_each(|felt| {
                    if let Ok(s) = parse_cairo_short_string(&FieldElement::from(*felt)) {
                        reasons.push(s);
                    }
                });

                tracing::warn!(target: "executor",
                               "Transaction validation error: {}", reasons.join(" "));
            }
            _ => tracing::warn!(target: "executor",
                                "Transaction validation error: {:?}", err),
        },
        _ => tracing::warn!(target: "executor",
                            "Transaction validation error: {:?}", err),
    }
}

pub(crate) fn pretty_print_resources(resources: &ResourcesMapping) -> String {
    let mut mapped_strings: Vec<_> = resources
        .0
        .iter()
        .filter_map(|(k, v)| match k.as_str() {
            "l1_gas_usage" => Some(format!("L1 Gas: {}", v)),
            "range_check_builtin" => Some(format!("Range Checks: {}", v)),
            "ecdsa_builtin" => Some(format!("ECDSA: {}", v)),
            "n_steps" => None,
            "pedersen_builtin" => Some(format!("Pedersen: {}", v)),
            "bitwise_builtin" => Some(format!("Bitwise: {}", v)),
            "keccak_builtin" => Some(format!("Keccak: {}", v)),
            _ => Some(format!("{}: {}", k.to_case(Case::Title), v)),
        })
        .collect::<Vec<String>>();

    // Sort the strings alphabetically
    mapped_strings.sort();

    // Prepend "Steps" if it exists, so it is always first
    if let Some(steps) = resources.0.get("n_steps") {
        mapped_strings.insert(0, format!("Steps: {}", steps));
    }

    mapped_strings.join(" | ")
}

pub fn get_state_update_from_cached_state(
    state: &CachedStateWrapper,
) -> StateUpdatesWithDeclaredClasses {
    let state_diff = state.inner().to_state_diff();

    let declared_sierra_classes = state.sierra_class().clone();

    let declared_compiled_classes = state_diff
        .class_hash_to_compiled_class_hash
        .iter()
        .map(|(class_hash, _)| {
            let class = state.class(class_hash.0.into()).unwrap().expect("must exist if declared");
            (class_hash.0.into(), class)
        })
        .collect::<HashMap<
            katana_primitives::contract::ClassHash,
            katana_primitives::contract::CompiledContractClass,
        >>();

    let nonce_updates =
        state_diff
            .address_to_nonce
            .into_iter()
            .map(|(key, value)| (key.into(), value.0.into()))
            .collect::<HashMap<
                katana_primitives::contract::ContractAddress,
                katana_primitives::contract::Nonce,
            >>();

    let storage_changes = state_diff
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

    let contract_updates = state_diff
        .address_to_class_hash
        .into_iter()
        .map(|(key, value)| (key.into(), value.0.into()))
        .collect::<HashMap<
            katana_primitives::contract::ContractAddress,
            katana_primitives::contract::ClassHash,
        >>();

    let declared_classes = state_diff
        .class_hash_to_compiled_class_hash
        .into_iter()
        .map(|(key, value)| (key.0.into(), value.0.into()))
        .collect::<HashMap<
            katana_primitives::contract::ClassHash,
            katana_primitives::contract::CompiledClassHash,
        >>();

    StateUpdatesWithDeclaredClasses {
        declared_sierra_classes,
        declared_compiled_classes,
        state_updates: StateUpdates {
            nonce_updates,
            storage_updates: storage_changes,
            contract_updates,
            declared_classes,
        },
    }
}

pub(super) fn trace_events(events: &[Event]) {
    for e in events {
        let formatted_keys =
            e.keys.iter().map(|k| format!("{k:#x}")).collect::<Vec<_>>().join(", ");

        trace!(target: "executor", "Event emitted keys=[{}]", formatted_keys);
    }
}

pub(super) fn events_from_exec_info(execution_info: &TransactionExecutionInfo) -> Vec<Event> {
    let mut events: Vec<Event> = vec![];

    fn get_events_recursively(call_info: &CallInfo) -> Vec<Event> {
        let mut events: Vec<Event> = vec![];

        events.extend(call_info.execution.events.iter().map(|e| Event {
            from_address: call_info.call.storage_address.into(),
            data: e.event.data.0.iter().map(|d| (*d).into()).collect(),
            keys: e.event.keys.iter().map(|k| k.0.into()).collect(),
        }));

        call_info.inner_calls.iter().for_each(|call| {
            events.extend(get_events_recursively(call));
        });

        events
    }

    if let Some(ref call) = execution_info.validate_call_info {
        events.extend(get_events_recursively(call));
    }

    if let Some(ref call) = execution_info.execute_call_info {
        events.extend(get_events_recursively(call));
    }

    if let Some(ref call) = execution_info.fee_transfer_call_info {
        events.extend(get_events_recursively(call));
    }

    events
}

pub(super) fn l2_to_l1_messages_from_exec_info(
    execution_info: &TransactionExecutionInfo,
) -> Vec<MessageToL1> {
    let mut messages = vec![];

    fn get_messages_recursively(info: &CallInfo) -> Vec<MessageToL1> {
        let mut messages = vec![];

        // By default, `from_address` must correspond to the contract address that
        // is sending the message. In the case of library calls, `code_address` is `None`,
        // we then use the `caller_address` instead (which can also be an account).
        let from_address = if let Some(code_address) = info.call.code_address {
            *code_address.0.key()
        } else {
            *info.call.caller_address.0.key()
        };

        messages.extend(info.execution.l2_to_l1_messages.iter().map(|m| MessageToL1 {
            to_address:
                FieldElement::from_byte_slice_be(m.message.to_address.0.as_bytes()).unwrap(),
            from_address: ContractAddress(from_address.into()),
            payload: m.message.payload.0.iter().map(|p| (*p).into()).collect(),
        }));

        info.inner_calls.iter().for_each(|call| {
            messages.extend(get_messages_recursively(call));
        });

        messages
    }

    if let Some(ref info) = execution_info.validate_call_info {
        messages.extend(get_messages_recursively(info));
    }

    if let Some(ref info) = execution_info.execute_call_info {
        messages.extend(get_messages_recursively(info));
    }

    if let Some(ref info) = execution_info.fee_transfer_call_info {
        messages.extend(get_messages_recursively(info));
    }

    messages
}

fn function_invocation_from_call_info(info: &CallInfo) -> FunctionInvocation {
    let entry_point_type = match info.call.entry_point_type {
        starknet_api::deprecated_contract_class::EntryPointType::Constructor => {
            starknet::core::types::EntryPointType::Constructor
        }
        starknet_api::deprecated_contract_class::EntryPointType::External => {
            starknet::core::types::EntryPointType::External
        }
        starknet_api::deprecated_contract_class::EntryPointType::L1Handler => {
            starknet::core::types::EntryPointType::L1Handler
        }
    };
    let call_type = match info.call.call_type {
        blockifier::execution::entry_point::CallType::Call => starknet::core::types::CallType::Call,
        blockifier::execution::entry_point::CallType::Delegate => {
            starknet::core::types::CallType::Delegate
        }
    };

    let calls = info.inner_calls.iter().map(function_invocation_from_call_info).collect();
    let events = info
        .execution
        .events
        .iter()
        .map(|e| starknet::core::types::OrderedEvent {
            order: e.order as u64,
            data: e.event.data.0.iter().map(|d| (*d).into()).collect(),
            keys: e.event.keys.iter().map(|k| k.0.into()).collect(),
        })
        .collect();
    let messages = info
        .execution
        .l2_to_l1_messages
        .iter()
        .map(|m| starknet::core::types::OrderedMessage {
            order: m.order as u64,
            to_address: (Into::<StarkFelt>::into(m.message.to_address)).into(),
            from_address: (*info.call.storage_address.0.key()).into(),
            payload: m.message.payload.0.iter().map(|p| (*p).into()).collect(),
        })
        .collect();

    let vm_resources = info.vm_resources.filter_unused_builtins();
    let get_vm_resource =
        |name: &str| vm_resources.builtin_instance_counter.get(name).map(|r| *r as u64);
    let execution_resources = starknet::core::types::ExecutionResources {
        steps: vm_resources.n_steps as u64,
        memory_holes: Some(vm_resources.n_memory_holes as u64),
        range_check_builtin_applications: get_vm_resource(RANGE_CHECK_BUILTIN_NAME),
        pedersen_builtin_applications: get_vm_resource(HASH_BUILTIN_NAME),
        poseidon_builtin_applications: get_vm_resource(POSEIDON_BUILTIN_NAME),
        ec_op_builtin_applications: get_vm_resource(EC_OP_BUILTIN_NAME),
        ecdsa_builtin_applications: get_vm_resource(SIGNATURE_BUILTIN_NAME),
        bitwise_builtin_applications: get_vm_resource(BITWISE_BUILTIN_NAME),
        keccak_builtin_applications: get_vm_resource(KECCAK_BUILTIN_NAME),
        segment_arena_builtin: get_vm_resource(SEGMENT_ARENA_BUILTIN_NAME),
    };

    FunctionInvocation {
        contract_address: (*info.call.storage_address.0.key()).into(),
        entry_point_selector: info.call.entry_point_selector.0.into(),
        calldata: info.call.calldata.0.iter().map(|f| (*f).into()).collect(),
        caller_address: (*info.call.caller_address.0.key()).into(),
        // See https://github.com/starkware-libs/blockifier/blob/main/crates/blockifier/src/execution/call_info.rs#L167
        class_hash: info.call.class_hash.expect("Class hash mut be set after execution").0.into(),
        entry_point_type,
        call_type,
        result: info.execution.retdata.0.iter().map(|f| (*f).into()).collect(),
        calls,
        events,
        messages,
        execution_resources,
    }
}

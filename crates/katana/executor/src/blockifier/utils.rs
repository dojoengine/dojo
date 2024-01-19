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
use blockifier::block_context::{FeeTokenAddresses, GasPrices};
use blockifier::fee::fee_utils::{calculate_l1_gas_by_vm_usage, extract_l1_gas_and_vm_usage};
use blockifier::state::state_api::State;
use blockifier::transaction::errors::TransactionExecutionError;
use blockifier::transaction::objects::{
    DeprecatedAccountTransactionContext, ResourcesMapping, TransactionExecutionInfo,
};
use convert_case::{Case, Casing};
use katana_primitives::contract::ContractAddress;
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::receipt::{Event, MessageToL1};
use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};
use katana_primitives::transaction::ExecutableTxWithHash;
use katana_primitives::FieldElement;
use katana_provider::traits::contract::ContractClassProvider;
use katana_provider::traits::state::StateProvider;
use starknet::core::types::FeeEstimate;
use starknet::core::utils::parse_cairo_short_string;
use starknet_api::block::{BlockNumber, BlockTimestamp};
use starknet_api::core::EntryPointSelector;
use starknet_api::transaction::Calldata;
use tracing::trace;

use super::state::{CachedStateWrapper, StateRefDb};
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
    let state = CachedStateWrapper::new(StateRefDb::from(state));
    let results = TransactionExecutor::new(&state, &block_context, false, validate, transactions)
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

/// Perform a raw entrypoint call of a contract.
pub fn raw_call(
    request: EntryPointCall,
    block_context: BlockContext,
    state: Box<dyn StateProvider>,
    initial_gas: u64,
) -> Result<CallInfo, TransactionExecutionError> {
    let mut state = CachedState::new(StateRefDb::from(state), GlobalContractCache::default());
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
    let gas_price = block_context.gas_prices.eth_l1_gas_price as u64;
    let gas_consumed = total_l1_gas_usage.ceil() as u64;
    let overall_fee = total_l1_gas_usage.ceil() as u64 * gas_price;

    Ok(FeeEstimate { gas_price, gas_consumed, overall_fee })
}

/// Create a block context from the chain environment values.
pub fn block_context_from_envs(block_env: &BlockEnv, cfg_env: &CfgEnv) -> BlockContext {
    let fee_token_addresses = FeeTokenAddresses {
        eth_fee_token_address: cfg_env.fee_token_addresses.eth.into(),
        strk_fee_token_address: cfg_env.fee_token_addresses.strk.into(),
    };

    let gas_prices = GasPrices {
        eth_l1_gas_price: block_env.l1_gas_prices.eth.try_into().unwrap(),
        strk_l1_gas_price: block_env.l1_gas_prices.strk.try_into().unwrap(),
    };

    BlockContext {
        gas_prices,
        fee_token_addresses,
        chain_id: cfg_env.chain_id.into(),
        block_number: BlockNumber(block_env.number),
        block_timestamp: BlockTimestamp(block_env.timestamp),
        sequencer_address: block_env.sequencer_address.into(),
        vm_resource_fee_cost: cfg_env.vm_resource_fee_cost.clone().into(),
        validate_max_n_steps: cfg_env.validate_max_n_steps,
        invoke_tx_max_n_steps: cfg_env.invoke_tx_max_n_steps,
        max_recursion_depth: cfg_env.max_recursion_depth,
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
    state: &CachedStateWrapper<StateRefDb>,
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

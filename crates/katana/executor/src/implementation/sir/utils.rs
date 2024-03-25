use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use katana_primitives::class::{CompiledClass, CompiledClassHash, DeprecatedCompiledClass};
use katana_primitives::contract::{ContractAddress, StorageKey, StorageValue};
use katana_primitives::env::{BlockEnv, CfgEnv};
use katana_primitives::fee::TxFeeInfo;
use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};
use katana_primitives::trace::TxExecInfo;
use katana_primitives::transaction::{
    DeployAccountTx, ExecutableTx, ExecutableTxWithHash, InvokeTx,
};
use katana_primitives::FieldElement;
use sir::definitions::block_context::{
    BlockContext, FeeTokenAddresses, FeeType, GasPrices, StarknetOsConfig,
};
use sir::definitions::constants::TRANSACTION_VERSION;
use sir::execution::execution_entry_point::ExecutionEntryPoint;
use sir::execution::{CallInfo, CallType, TransactionExecutionContext, TransactionExecutionInfo};
use sir::services::api::contract_classes::compiled_class::CompiledClass as SirCompiledClass;
use sir::services::api::contract_classes::deprecated_contract_class::ContractClass as SirDeprecatedContractClass;
use sir::state::contract_class_cache::{ContractClassCache, PermanentContractClassCache};
use sir::state::state_api::StateReader;
use sir::state::state_cache::StateCache;
use sir::state::{cached_state, BlockInfo, ExecutionResourcesManager, StateDiff};
use sir::transaction::error::TransactionError;
use sir::transaction::fee::{calculate_tx_fee, calculate_tx_l1_gas_usage};
use sir::transaction::{
    Address, ClassHash, CurrentAccountTxFields, DataAvailabilityMode, Declare, DeclareDeprecated,
    DeployAccount, InvokeFunction, L1Handler, ResourceBounds, Transaction,
    VersionSpecificAccountTxFields,
};
use sir::utils::calculate_sn_keccak;
use sir::EntryPointType;
use starknet::core::types::PriceUnit;
use starknet_types_core::felt::Felt;

use super::state::{CachedState, StateDb};
use super::SimulationFlag;
use crate::{EntryPointCall, ExecutionError};

pub(super) fn transact<S, C>(
    tx: ExecutableTxWithHash,
    state: &mut cached_state::CachedState<S, C>,
    block_context: &BlockContext,
    simulation_flag: &SimulationFlag,
) -> Result<(TransactionExecutionInfo, TxFeeInfo), ExecutionError>
where
    S: StateReader,
    C: ContractClassCache,
{
    let tx = to_executor_tx(tx, simulation_flag)?;
    let fee_type = tx.fee_type();

    let info = tx.execute(
        state,
        block_context,
        u128::MAX, // TODO: this should be set as part of the transaction fee
        #[cfg(feature = "native")]
        None,
    )?;

    // There are a few case where the `actual_fee` field of the transaction info is not set where
    // the fee is skipped and thus not charged for the transaction (e.g. when the
    // `skip_fee_transfer` is explicitly set, or when the transaction `max_fee` is set to 0). In
    // these cases, we still want to calculate the fee.
    let overall_fee = if info.actual_fee == 0 {
        calculate_tx_fee(&info.actual_resources, block_context, &fee_type)?
    } else {
        info.actual_fee
    };

    let gas_consumed = calculate_tx_l1_gas_usage(&info.actual_resources, block_context)?;
    let (unit, gas_price) = match fee_type {
        FeeType::Eth => (PriceUnit::Wei, block_context.get_gas_price_by_fee_type(&FeeType::Eth)),
        FeeType::Strk => (PriceUnit::Fri, block_context.get_gas_price_by_fee_type(&FeeType::Strk)),
    };
    let fee = TxFeeInfo { gas_consumed, gas_price, unit, overall_fee };

    Ok((info, fee))
}

pub fn call(
    request: EntryPointCall,
    state: impl StateReader,
    block_context: &BlockContext,
    initial_gas: u128,
) -> Result<Vec<FieldElement>, ExecutionError> {
    let mut state = cached_state::CachedState::new(
        Arc::new(state),
        Arc::new(PermanentContractClassCache::default()),
    );

    let contract_address = to_sir_address(&request.contract_address);
    let entry_point_selector = to_sir_felt(&request.entry_point_selector);
    let calldata = request.calldata.iter().map(to_sir_felt).collect::<Vec<Felt>>();
    let call_type = Some(CallType::Call);
    let caller_address = Address::default();
    let entry_point_type = EntryPointType::External;

    let call = ExecutionEntryPoint::new(
        contract_address,
        calldata,
        entry_point_selector,
        caller_address,
        entry_point_type,
        call_type,
        None,
        initial_gas,
    );

    let max_steps = block_context.invoke_tx_max_n_steps();
    let mut resources_manager = ExecutionResourcesManager::default();
    let mut tx_execution_context = TransactionExecutionContext::new(
        Address::default(),
        Felt::default(),
        Vec::new(),
        Default::default(),
        Felt::default(),
        block_context.invoke_tx_max_n_steps(),
        *TRANSACTION_VERSION,
    );

    let result = call.execute(
        &mut state,
        block_context,
        &mut resources_manager,
        &mut tx_execution_context,
        false,
        max_steps,
        #[cfg(feature = "native")]
        None,
    )?;

    let info = result.call_info.expect("should exist in call result");
    let retdata = info.retdata.iter().map(to_felt).collect();

    Ok(retdata)
}

fn to_executor_tx(
    katana_tx: ExecutableTxWithHash,
    simulation_flag: &SimulationFlag,
) -> Result<Transaction, TransactionError> {
    match katana_tx.transaction {
        ExecutableTx::Invoke(tx) => match tx {
            InvokeTx::V1(tx) => {
                let version = Felt::ONE;
                let contract_address = to_sir_address(&tx.sender_address);
                let entry_point = Felt::from_bytes_be(&calculate_sn_keccak(b"__execute__"));
                let ver_specifc_fields = VersionSpecificAccountTxFields::Deprecated(tx.max_fee);
                let calldata = tx.calldata.iter().map(to_sir_felt).collect::<Vec<Felt>>();
                let signature = tx.signature.iter().map(to_sir_felt).collect::<Vec<Felt>>();
                let nonce = Some(to_sir_felt(&tx.nonce));
                let tx_hash = to_sir_felt(&katana_tx.hash);

                let tx = InvokeFunction::new_with_tx_hash(
                    contract_address,
                    entry_point,
                    ver_specifc_fields,
                    version,
                    calldata,
                    signature,
                    nonce,
                    tx_hash,
                )?;

                let tx = tx.create_for_simulation(
                    simulation_flag.skip_validate,
                    simulation_flag.skip_execute,
                    simulation_flag.skip_fee_transfer,
                    simulation_flag.ignore_max_fee,
                    simulation_flag.skip_nonce_check,
                );

                Ok(tx)
            }

            InvokeTx::V3(tx) => {
                let version = Felt::THREE;
                let contract_address = to_sir_address(&tx.sender_address);
                let entry_point = Felt::from_bytes_be(&calculate_sn_keccak(b"__execute__"));

                let ver_specifc_fields = to_sir_current_account_tx_fields(
                    tx.tip,
                    tx.resource_bounds.l1_gas,
                    tx.resource_bounds.l2_gas,
                    tx.nonce_data_availability_mode,
                    tx.fee_data_availability_mode,
                    tx.paymaster_data,
                    tx.account_deployment_data,
                );

                let calldata = tx.calldata.iter().map(to_sir_felt).collect::<Vec<Felt>>();
                let signature = tx.signature.iter().map(to_sir_felt).collect::<Vec<Felt>>();
                let nonce = Some(to_sir_felt(&tx.nonce));
                let tx_hash = to_sir_felt(&katana_tx.hash);

                let tx = InvokeFunction::new_with_tx_hash(
                    contract_address,
                    entry_point,
                    ver_specifc_fields,
                    version,
                    calldata,
                    signature,
                    nonce,
                    tx_hash,
                )?;

                let tx = tx.create_for_simulation(
                    simulation_flag.skip_validate,
                    simulation_flag.skip_execute,
                    simulation_flag.skip_fee_transfer,
                    simulation_flag.ignore_max_fee,
                    simulation_flag.skip_nonce_check,
                );

                Ok(tx)
            }
        },

        ExecutableTx::DeployAccount(tx) => match tx {
            DeployAccountTx::V1(tx) => {
                let version = Felt::ONE;
                let class_hash = to_sir_class_hash(&tx.class_hash);
                let ver_specifc_fields = VersionSpecificAccountTxFields::Deprecated(tx.max_fee);
                let calldata = tx.constructor_calldata.iter().map(to_sir_felt).collect();
                let signature = tx.signature.iter().map(to_sir_felt).collect::<Vec<Felt>>();
                let nonce = to_sir_felt(&tx.nonce);
                let tx_hash = to_sir_felt(&katana_tx.hash);

                let tx = DeployAccount::new_with_tx_hash(
                    class_hash,
                    ver_specifc_fields,
                    version,
                    nonce,
                    calldata,
                    signature,
                    nonce,
                    tx_hash,
                )?;

                let tx = tx.create_for_simulation(
                    simulation_flag.skip_validate,
                    simulation_flag.skip_execute,
                    simulation_flag.skip_fee_transfer,
                    simulation_flag.ignore_max_fee,
                    simulation_flag.skip_nonce_check,
                );

                Ok(tx)
            }

            DeployAccountTx::V3(tx) => {
                let version = Felt::THREE;

                let class_hash = to_sir_class_hash(&tx.class_hash);
                let ver_specifc_fields = to_sir_current_account_tx_fields(
                    tx.tip,
                    tx.resource_bounds.l1_gas,
                    tx.resource_bounds.l2_gas,
                    tx.nonce_data_availability_mode,
                    tx.fee_data_availability_mode,
                    tx.paymaster_data,
                    vec![],
                );
                let calldata = tx.constructor_calldata.iter().map(to_sir_felt).collect();
                let signature = tx.signature.iter().map(to_sir_felt).collect::<Vec<Felt>>();
                let nonce = to_sir_felt(&tx.nonce);
                let tx_hash = to_sir_felt(&katana_tx.hash);

                let tx = DeployAccount::new_with_tx_hash(
                    class_hash,
                    ver_specifc_fields,
                    version,
                    nonce,
                    calldata,
                    signature,
                    nonce,
                    tx_hash,
                )?;

                let tx = tx.create_for_simulation(
                    simulation_flag.skip_validate,
                    simulation_flag.skip_execute,
                    simulation_flag.skip_fee_transfer,
                    simulation_flag.ignore_max_fee,
                    simulation_flag.skip_nonce_check,
                );

                Ok(tx)
            }
        },

        ExecutableTx::Declare(declare) => match declare.transaction {
            katana_primitives::transaction::DeclareTx::V1(tx) => {
                let sender_address = to_sir_address(&tx.sender_address);
                let max_fee = tx.max_fee;
                let version = Felt::ONE;
                let signature = tx.signature.iter().map(to_sir_felt).collect::<Vec<Felt>>();
                let nonce = to_sir_felt(&tx.nonce);
                let tx_hash = to_sir_felt(&katana_tx.hash);
                let class_hash = to_sir_class_hash(&tx.class_hash);

                let CompiledClass::Deprecated(class) = declare.compiled_class else { panic!() };
                let contract_class = to_sir_deprecated_class(class.clone());

                let tx = DeclareDeprecated::new_with_tx_and_class_hash(
                    contract_class,
                    sender_address,
                    max_fee,
                    version,
                    signature,
                    nonce,
                    tx_hash,
                    class_hash,
                )?;

                let tx = tx.create_for_simulation(
                    simulation_flag.skip_validate,
                    simulation_flag.skip_execute,
                    simulation_flag.skip_fee_transfer,
                    simulation_flag.ignore_max_fee,
                    simulation_flag.skip_nonce_check,
                );

                Ok(tx)
            }

            katana_primitives::transaction::DeclareTx::V2(tx) => {
                let sierra_contract_class = None;
                let sierra_class_hash = to_sir_felt(&tx.class_hash);
                let compiled_class_hash = to_sir_felt(&tx.compiled_class_hash);
                let sender_address = to_sir_address(&tx.sender_address);
                let account_tx_fields = VersionSpecificAccountTxFields::Deprecated(tx.max_fee);
                let version = Felt::TWO;
                let signature = tx.signature.iter().map(to_sir_felt).collect::<Vec<Felt>>();
                let nonce = to_sir_felt(&tx.nonce);
                let tx_hash = to_sir_felt(&katana_tx.hash);

                let CompiledClass::Class(class) = declare.compiled_class else { panic!() };
                let casm_contract_class = Some(class.casm.clone());

                let tx = Declare::new_with_sierra_class_hash_and_tx_hash(
                    sierra_contract_class,
                    sierra_class_hash,
                    casm_contract_class,
                    compiled_class_hash,
                    sender_address,
                    account_tx_fields,
                    version,
                    signature,
                    nonce,
                    tx_hash,
                )?;

                let tx = tx.create_for_simulation(
                    simulation_flag.skip_validate,
                    simulation_flag.skip_execute,
                    simulation_flag.skip_fee_transfer,
                    simulation_flag.ignore_max_fee,
                    simulation_flag.skip_nonce_check,
                );

                Ok(tx)
            }

            katana_primitives::transaction::DeclareTx::V3(tx) => {
                let sierra_contract_class = None;
                let sierra_class_hash = to_sir_felt(&tx.class_hash);
                let compiled_class_hash = to_sir_felt(&tx.compiled_class_hash);
                let sender_address = to_sir_address(&tx.sender_address);
                let ver_specifc_fields = to_sir_current_account_tx_fields(
                    tx.tip,
                    tx.resource_bounds.l1_gas,
                    tx.resource_bounds.l2_gas,
                    tx.nonce_data_availability_mode,
                    tx.fee_data_availability_mode,
                    tx.paymaster_data,
                    tx.account_deployment_data,
                );
                let version = Felt::THREE;
                let signature = tx.signature.iter().map(to_sir_felt).collect::<Vec<Felt>>();
                let nonce = to_sir_felt(&tx.nonce);
                let tx_hash = to_sir_felt(&katana_tx.hash);

                let CompiledClass::Class(class) = declare.compiled_class else { panic!() };
                let casm_contract_class = Some(class.casm.clone());

                let tx = Declare::new_with_sierra_class_hash_and_tx_hash(
                    sierra_contract_class,
                    sierra_class_hash,
                    casm_contract_class,
                    compiled_class_hash,
                    sender_address,
                    ver_specifc_fields,
                    version,
                    signature,
                    nonce,
                    tx_hash,
                )?;

                let tx = tx.create_for_simulation(
                    simulation_flag.skip_validate,
                    simulation_flag.skip_execute,
                    simulation_flag.skip_fee_transfer,
                    simulation_flag.ignore_max_fee,
                    simulation_flag.skip_nonce_check,
                );

                Ok(tx)
            }
        },

        ExecutableTx::L1Handler(tx) => {
            let contract_address = to_sir_address(&tx.contract_address);
            let entry_point = to_sir_felt(&tx.entry_point_selector);
            let calldata = tx.calldata.iter().map(to_sir_felt).collect::<Vec<Felt>>();
            let nonce = to_sir_felt(&tx.nonce);
            let paid_fee_on_l1 = Some(Felt::from(tx.paid_fee_on_l1));
            let tx_hash = to_sir_felt(&katana_tx.hash);

            let tx = L1Handler::new_with_tx_hash(
                contract_address,
                entry_point,
                calldata,
                nonce,
                paid_fee_on_l1,
                tx_hash,
            )?;

            let tx = tx
                .create_for_simulation(simulation_flag.skip_validate, simulation_flag.skip_execute);

            Ok(tx)
        }
    }
}

fn state_diff_from_state_cache(mut cache: StateCache) -> StateDiff {
    let address_to_class_hash = std::mem::take(cache.class_hash_writes_mut());
    let address_to_nonce = std::mem::take(cache.nonce_writes_mut());
    let class_hash_to_compiled_class = std::mem::take(cache.compiled_class_hash_writes_mut());
    let storage_updates = sir::utils::to_state_diff_storage_mapping(cache.storage_writes());

    StateDiff::new(
        address_to_class_hash,
        address_to_nonce,
        class_hash_to_compiled_class,
        storage_updates,
    )
}

pub(super) fn to_felt(value: &Felt) -> FieldElement {
    FieldElement::from_bytes_be(&value.to_bytes_be()).unwrap()
}

pub(super) fn to_sir_felt(value: &FieldElement) -> Felt {
    Felt::from_bytes_be(&value.to_bytes_be())
}

pub(super) fn to_address(value: &Address) -> ContractAddress {
    ContractAddress::new(FieldElement::from_bytes_be(&value.0.to_bytes_be()).unwrap())
}

pub(super) fn to_sir_address(value: &ContractAddress) -> Address {
    Address(to_sir_felt(&value.0))
}

pub(super) fn to_class_hash(value: &ClassHash) -> katana_primitives::class::ClassHash {
    FieldElement::from_bytes_be(&value.0).unwrap()
}

pub(super) fn to_sir_class_hash(value: &katana_primitives::class::ClassHash) -> ClassHash {
    ClassHash(value.to_bytes_be())
}

pub(super) fn to_sir_compiled_class(class: CompiledClass) -> SirCompiledClass {
    match class {
        CompiledClass::Class(class) => {
            let casm = Arc::new(class.casm);
            let sierra = Some(Arc::new((class.sierra.program, class.sierra.entry_points_by_type)));
            SirCompiledClass::Casm { casm, sierra }
        }

        CompiledClass::Deprecated(class) => {
            let class = Arc::new(to_sir_deprecated_class(class));
            SirCompiledClass::Deprecated(class)
        }
    }
}

pub(super) fn to_sir_deprecated_class(
    class: DeprecatedCompiledClass,
) -> SirDeprecatedContractClass {
    let json = serde_json::to_string(&class).unwrap();
    SirDeprecatedContractClass::from_str(&json).unwrap()
}

fn to_sir_current_account_tx_fields(
    tip: u64,
    l1_gas_resource_bounds: starknet::core::types::ResourceBounds,
    l2_gas_resource_bounds: starknet::core::types::ResourceBounds,
    nonce_data_availability_mode: starknet::core::types::DataAvailabilityMode,
    fee_data_availability_mode: starknet::core::types::DataAvailabilityMode,
    paymaster_data: Vec<FieldElement>,
    account_deployment_data: Vec<FieldElement>,
) -> VersionSpecificAccountTxFields {
    fn to_sir_da_mode(mode: starknet::core::types::DataAvailabilityMode) -> DataAvailabilityMode {
        match mode {
            starknet::core::types::DataAvailabilityMode::L1 => DataAvailabilityMode::L1,
            starknet::core::types::DataAvailabilityMode::L2 => DataAvailabilityMode::L2,
        }
    }

    fn to_sir_resource_bounds(
        resource_bounds: starknet::core::types::ResourceBounds,
    ) -> ResourceBounds {
        ResourceBounds {
            max_amount: resource_bounds.max_amount,
            max_price_per_unit: resource_bounds.max_price_per_unit,
        }
    }

    let l1_resource_bounds = to_sir_resource_bounds(l1_gas_resource_bounds);
    let l2_resource_bounds = Some(to_sir_resource_bounds(l2_gas_resource_bounds));
    let nonce_data_availability_mode = to_sir_da_mode(nonce_data_availability_mode);
    let fee_data_availability_mode = to_sir_da_mode(fee_data_availability_mode);
    let paymaster_data = paymaster_data.iter().map(to_sir_felt).collect::<Vec<Felt>>();
    let account_deployment_data =
        account_deployment_data.iter().map(to_sir_felt).collect::<Vec<Felt>>();

    VersionSpecificAccountTxFields::Current(CurrentAccountTxFields {
        tip,
        paymaster_data,
        l1_resource_bounds,
        l2_resource_bounds,
        account_deployment_data,
        fee_data_availability_mode,
        nonce_data_availability_mode,
    })
}

pub fn to_exec_info(exec_info: &TransactionExecutionInfo) -> TxExecInfo {
    TxExecInfo {
        validate_call_info: exec_info.validate_info.clone().map(from_sir_call_info),
        execute_call_info: exec_info.call_info.clone().map(from_sir_call_info),
        fee_transfer_call_info: exec_info.fee_transfer_info.clone().map(from_sir_call_info),
        actual_fee: exec_info.actual_fee,
        actual_resources: exec_info
            .actual_resources
            .clone()
            .into_iter()
            .map(|(k, v)| (k, v as u64))
            .collect(),
        revert_error: exec_info.revert_error.clone(),
        // exec_info.tx_type being dropped here.
    }
}

fn from_sir_call_info(call_info: CallInfo) -> katana_primitives::trace::CallInfo {
    let message_to_l1_from_address = if let Some(ref a) = call_info.code_address {
        to_address(a)
    } else {
        to_address(&call_info.caller_address)
    };

    katana_primitives::trace::CallInfo {
        contract_address: to_address(&call_info.contract_address),
        caller_address: to_address(&call_info.caller_address),
        call_type: match call_info.call_type {
            Some(CallType::Call) => katana_primitives::trace::CallType::Call,
            Some(CallType::Delegate) => katana_primitives::trace::CallType::Delegate,
            _ => panic!("CallType is expected"),
        },
        code_address: call_info.code_address.as_ref().map(to_address),
        class_hash: call_info.class_hash.as_ref().map(to_class_hash),
        entry_point_selector: to_felt(
            &call_info.entry_point_selector.expect("EntryPointSelector is expected"),
        ),
        entry_point_type: match call_info.entry_point_type {
            Some(EntryPointType::External) => katana_primitives::trace::EntryPointType::External,
            Some(EntryPointType::L1Handler) => katana_primitives::trace::EntryPointType::L1Handler,
            Some(EntryPointType::Constructor) => {
                katana_primitives::trace::EntryPointType::Constructor
            }
            _ => panic!("EntryPointType is expected"),
        },
        calldata: call_info.calldata.iter().map(to_felt).collect(),
        retdata: call_info.retdata.iter().map(to_felt).collect(),
        execution_resources: if let Some(ei) = call_info.execution_resources {
            katana_primitives::trace::ExecutionResources {
                n_steps: ei.n_steps as u64,
                n_memory_holes: ei.n_memory_holes as u64,
                builtin_instance_counter: ei
                    .builtin_instance_counter
                    .into_iter()
                    .map(|(k, v)| (k, v as u64))
                    .collect(),
            }
        } else {
            katana_primitives::trace::ExecutionResources::default()
        },
        events: call_info
            .events
            .iter()
            .map(|e| katana_primitives::event::OrderedEvent {
                order: e.order,
                keys: e.keys.iter().map(to_felt).collect(),
                data: e.data.iter().map(to_felt).collect(),
            })
            .collect(),
        l2_to_l1_messages: call_info
            .l2_to_l1_messages
            .iter()
            .map(|m| katana_primitives::message::OrderedL2ToL1Message {
                order: m.order as u64,
                from_address: message_to_l1_from_address,
                to_address: *to_address(&m.to_address),
                payload: m.payload.iter().map(to_felt).collect(),
            })
            .collect(),
        storage_read_values: call_info
            .storage_read_values
            .into_iter()
            .map(|f| to_felt(&f))
            .collect(),
        accessed_storage_keys: call_info.accessed_storage_keys.iter().map(to_class_hash).collect(),
        inner_calls: call_info
            .internal_calls
            .iter()
            .map(|c| from_sir_call_info(c.clone()))
            .collect(),
        gas_consumed: call_info.gas_consumed,
        failed: call_info.failure_flag,
    }
}

pub(super) fn block_context_from_envs(block_env: &BlockEnv, cfg_env: &CfgEnv) -> BlockContext {
    let chain_id = to_sir_felt(&cfg_env.chain_id.id());
    let fee_token_addreses = FeeTokenAddresses::new(
        to_sir_address(&cfg_env.fee_token_addresses.eth),
        to_sir_address(&cfg_env.fee_token_addresses.strk),
    );

    let gas_price = GasPrices {
        eth_l1_gas_price: block_env.l1_gas_prices.eth,
        strk_l1_gas_price: block_env.l1_gas_prices.strk,
    };

    let block_info = BlockInfo {
        gas_price,
        block_number: block_env.number,
        block_timestamp: block_env.timestamp,
        sequencer_address: to_sir_address(&block_env.sequencer_address),
    };

    BlockContext::new(
        StarknetOsConfig::new(chain_id, fee_token_addreses),
        Default::default(),
        Default::default(),
        cfg_env.vm_resource_fee_cost.clone(),
        cfg_env.invoke_tx_max_n_steps as u64,
        cfg_env.validate_max_n_steps as u64,
        block_info,
        Default::default(),
        false,
    )
}
pub(super) fn state_update_from_cached_state<S, C>(
    state: &CachedState<S, C>,
) -> StateUpdatesWithDeclaredClasses
where
    S: StateDb,
    C: ContractClassCache + Send + Sync,
{
    use katana_primitives::class::ClassHash;

    let state = &mut state.0.write();
    let state_changes = std::mem::take(state.inner.cache_mut());
    let state_diffs = state_diff_from_state_cache(state_changes);
    let compiled_classes = std::mem::take(&mut state.declared_classes);

    let nonce_updates: HashMap<ContractAddress, FieldElement> =
        state_diffs.address_to_nonce().iter().map(|(k, v)| (to_address(k), to_felt(v))).collect();

    let declared_classes: HashMap<ClassHash, CompiledClassHash> = state_diffs
        .class_hash_to_compiled_class()
        .iter()
        .map(|(k, v)| (to_class_hash(k), to_class_hash(v)))
        .collect();

    let contract_updates: HashMap<ContractAddress, ClassHash> = state_diffs
        .address_to_class_hash()
        .iter()
        .map(|(k, v)| (to_address(k), to_class_hash(v)))
        .collect();

    let storage_updates: HashMap<ContractAddress, HashMap<StorageKey, StorageValue>> = state_diffs
        .storage_updates()
        .iter()
        .map(|(k, v)| {
            let k = to_address(k);
            let v = v.iter().map(|(k, v)| (to_felt(k), to_felt(v))).collect();
            (k, v)
        })
        .collect();

    let total_classes = declared_classes.len();
    let mut declared_compiled_classes = HashMap::with_capacity(total_classes);
    let mut declared_sierra_classes = HashMap::with_capacity(total_classes);

    for (hash, (compiled, sierra)) in compiled_classes {
        declared_compiled_classes.insert(hash, compiled);
        if let Some(sierra) = sierra {
            declared_sierra_classes.insert(hash, sierra);
        }
    }

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

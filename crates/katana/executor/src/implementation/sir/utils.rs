use std::str::FromStr;
use std::sync::Arc;

use katana_primitives::class::{CompiledClass, DeprecatedCompiledClass};
use katana_primitives::contract::ContractAddress;
use katana_primitives::transaction::{
    DeployAccountTx, ExecutableTx, ExecutableTxWithHash, InvokeTx,
};
use katana_primitives::FieldElement;
use sir::definitions::block_context::{BlockContext, FeeType};
use sir::definitions::constants::TRANSACTION_VERSION;
use sir::execution::execution_entry_point::{ExecutionEntryPoint, ExecutionResult};
use sir::execution::{CallType, TransactionExecutionContext};
use sir::services::api::contract_classes::compiled_class::CompiledClass as SirCompiledClass;
use sir::services::api::contract_classes::deprecated_contract_class::ContractClass as SirDeprecatedContractClass;
use sir::state::contract_class_cache::ContractClassCache;
use sir::state::state_api::StateReader;
use sir::state::state_cache::StateCache;
use sir::state::{cached_state, ExecutionResourcesManager, StateDiff};
use sir::transaction::error::TransactionError;
use sir::transaction::fee;
use sir::transaction::{
    Address, ClassHash, CurrentAccountTxFields, DataAvailabilityMode, Declare, DeclareDeprecated,
    DeployAccount, InvokeFunction, L1Handler, ResourceBounds, Transaction,
    VersionSpecificAccountTxFields,
};
use sir::utils::calculate_sn_keccak;
use sir::EntryPointType;
use starknet::core::types::PriceUnit;
use starknet_types_core::felt::Felt;

use super::output::TransactionExecutionInfo;
use super::state::StateDb;
use super::SimulationFlag;
use crate::{EntryPointCall, TxFee};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to execute transaction: {0}")]
    TransactionError(#[from] sir::transaction::error::TransactionError),
}

pub(super) fn transact<S, C>(
    tx: ExecutableTxWithHash,
    state: &mut cached_state::CachedState<S, C>,
    block_context: &BlockContext,
    remaining_gas: u128,
    simulation_flag: &SimulationFlag,
) -> Result<TransactionExecutionInfo, Error>
where
    S: StateReader,
    C: ContractClassCache,
{
    let (tx, fee_type) = to_executor_tx_and_fee_type(tx, simulation_flag)?;
    let info = tx.execute(
        state,
        block_context,
        remaining_gas,
        #[cfg(feature = "native")]
        None,
    )?;

    let l1_gas_usage = fee::calculate_tx_l1_gas_usage(&info.actual_resources, block_context)?;

    // There are a few case where the `actual_fee` field of the transaction info is not set where
    // the fee is skipped and thus not charged for the transaction (e.g. when the
    // `skip_fee_transfer` is explicitly set, or when the transaction `max_fee` is set to 0). In
    // these cases, we still want to calculate the fee.
    let fee = if info.actual_fee == 0 {
        l1_gas_usage * block_context.get_gas_price_by_fee_type(&fee_type)
    } else {
        info.actual_fee
    };

    let (unit, gas_price) = match fee_type {
        FeeType::Eth => (PriceUnit::Wei, block_context.block_info().gas_price.eth_l1_gas_price),
        FeeType::Strk => (PriceUnit::Fri, block_context.block_info().gas_price.strk_l1_gas_price),
    };

    let fee_info = TxFee { unit, gas_price, fee, gas_consumed: l1_gas_usage };

    Ok(TransactionExecutionInfo::new(info, fee_info))
}

pub(super) fn call<S, C>(
    params: EntryPointCall,
    state: &mut cached_state::CachedState<S, C>,
    block_context: &BlockContext,
    initial_gas: u128,
) -> Result<ExecutionResult, Error>
where
    S: StateDb + Send + Sync,
    C: ContractClassCache + Send + Sync,
{
    // let state_reader = Arc::new(state);
    // let contract_classes = Arc::new(PermanentContractClassCache::default());
    // let mut state = cached_state::CachedState::new(state_reader, contract_classes);

    let contract_address = to_sir_address(&params.contract_address);
    let entry_point_selector = to_sir_felt(&params.entry_point_selector);
    let calldata = params.calldata.iter().map(to_sir_felt).collect::<Vec<Felt>>();
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
        state,
        block_context,
        &mut resources_manager,
        &mut tx_execution_context,
        false,
        max_steps,
        #[cfg(feature = "native")]
        None,
    )?;

    Ok(result)
}

fn to_executor_tx_and_fee_type(
    katana_tx: ExecutableTxWithHash,
    simulation_flag: &SimulationFlag,
) -> Result<(Transaction, FeeType), TransactionError> {
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

                Ok((tx, FeeType::Eth))
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

                Ok((tx, FeeType::Strk))
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

                Ok((tx, FeeType::Eth))
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

                Ok((tx, FeeType::Strk))
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

                Ok((tx, FeeType::Eth))
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

                Ok((tx, FeeType::Eth))
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

                Ok((tx, FeeType::Strk))
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

            Ok((tx, FeeType::Eth))
        }
    }
}

pub(super) fn state_diff_from_state_cache(mut cache: StateCache) -> StateDiff {
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

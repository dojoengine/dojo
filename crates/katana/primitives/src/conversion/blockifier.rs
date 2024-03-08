//! Translation layer for converting the primitive types to the execution engine types.

use blockifier::execution::call_info::CallInfo as BlockifierCallInfo;
use blockifier::execution::contract_class::{ContractClass, ContractClassV0, ContractClassV1};
use blockifier::execution::entry_point::CallType as BlockifierCallType;
use blockifier::transaction::objects::TransactionExecutionInfo;
use cairo_vm::types::errors::program_errors::ProgramError;
use cairo_vm::vm::runners::cairo_runner::ExecutionResources;
use starknet::core::utils::parse_cairo_short_string;
use starknet_api::core::{ContractAddress, PatriciaKey};
use starknet_api::deprecated_contract_class::EntryPointType as BlockifierEntryPointType;
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;

use crate::chain::ChainId;
use crate::class::CompiledClass;
use crate::event::OrderedEvent;
use crate::message::OrderedL2ToL1Message;
use crate::trace::{CallInfo, CallType, EntryPointType, TxExecInfo};
use crate::FieldElement;

impl From<crate::contract::ContractAddress> for ContractAddress {
    fn from(address: crate::contract::ContractAddress) -> Self {
        Self(patricia_key!(address.0))
    }
}

impl From<ContractAddress> for crate::contract::ContractAddress {
    fn from(address: ContractAddress) -> Self {
        Self((*address.0.key()).into())
    }
}

impl From<ChainId> for starknet_api::core::ChainId {
    fn from(chain_id: ChainId) -> Self {
        let name: String = match chain_id {
            ChainId::Named(named) => named.name().to_string(),
            ChainId::Id(id) => parse_cairo_short_string(&id).expect("valid cairo string"),
        };
        Self(name)
    }
}

pub fn to_class(class: CompiledClass) -> Result<ContractClass, ProgramError> {
    match class {
        CompiledClass::Deprecated(class) => {
            Ok(ContractClass::V0(ContractClassV0::try_from(class)?))
        }

        CompiledClass::Class(class) => {
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

/// Currently only blockifier -> primitive is implemented as the traces
/// are not sent back to the blockifier.
impl From<TransactionExecutionInfo> for TxExecInfo {
    fn from(v: TransactionExecutionInfo) -> Self {
        Self {
            validate_call_info: v.validate_call_info.map(|ci| ci.into()),
            execute_call_info: v.execute_call_info.map(|ci| ci.into()),
            fee_transfer_call_info: v.fee_transfer_call_info.map(|ci| ci.into()),
            actual_fee: v.actual_fee.0,
            actual_resources: v
                .actual_resources
                .0
                .into_iter()
                .map(|(k, v)| (k, v as u64))
                .collect(),
            revert_error: v.revert_error.map(|s| s.clone()),
        }
    }
}

impl From<BlockifierCallInfo> for CallInfo {
    fn from(v: BlockifierCallInfo) -> Self {
        let message_to_l1_from_address =
            if let Some(a) = v.call.code_address { a.into() } else { v.call.caller_address.into() };

        Self {
            caller_address: v.call.caller_address.into(),
            call_type: match v.call.call_type {
                BlockifierCallType::Call => CallType::Call,
                BlockifierCallType::Delegate => CallType::Delegate,
            },
            code_address: v.call.code_address.map(|a| a.into()),
            class_hash: v.call.class_hash.map(|a| a.0.into()),
            entry_point_selector: v.call.entry_point_selector.0.into(),
            entry_point_type: match v.call.entry_point_type {
                BlockifierEntryPointType::External => EntryPointType::External,
                BlockifierEntryPointType::L1Handler => EntryPointType::L1Handler,
                BlockifierEntryPointType::Constructor => EntryPointType::Constructor,
            },
            calldata: v.call.calldata.0.iter().map(|f| (*f).into()).collect(),
            retdata: v.execution.retdata.0.iter().map(|f| (*f).into()).collect(),
            execution_resources: ExecutionResources {
                n_steps: v.vm_resources.n_steps,
                n_memory_holes: v.vm_resources.n_memory_holes,
                builtin_instance_counter: v.vm_resources.builtin_instance_counter.clone(),
            },
            events: v
                .execution
                .events
                .iter()
                .map(|e| OrderedEvent {
                    order: e.order as u64,
                    keys: e.event.keys.iter().map(|f| f.0.into()).collect(),
                    data: e.event.data.0.iter().map(|f| (*f).into()).collect(),
                })
                .collect(),
            l2_to_l1_messages: v
                .execution
                .l2_to_l1_messages
                .iter()
                .map(|m| {
                    let to_address = starknet_api_ethaddr_to_felt(m.message.to_address);
                    OrderedL2ToL1Message {
                        order: m.order as u64,
                        from_address: message_to_l1_from_address,
                        to_address: to_address.into(),
                        payload: m.message.payload.0.iter().map(|f| (*f).into()).collect(),
                    }
                })
                .collect(),
            storage_read_values: v.storage_read_values.into_iter().map(|f| f.into()).collect(),
            accessed_storage_keys: v
                .accessed_storage_keys
                .into_iter()
                .map(|sk| (*sk.0.key()).into())
                .collect(),
            inner_calls: v.inner_calls.iter().map(|c| c.clone().into()).collect(),
            gas_consumed: v.execution.gas_consumed as u128,
            failed: v.execution.failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::utils::parse_cairo_short_string;

    use crate::chain::{ChainId, NamedChainId};

    #[test]
    fn convert_chain_id() {
        let mainnet = starknet_api::core::ChainId::from(ChainId::Named(NamedChainId::Mainnet));
        let goerli = starknet_api::core::ChainId::from(ChainId::Named(NamedChainId::Goerli));
        let sepolia = starknet_api::core::ChainId::from(ChainId::Named(NamedChainId::Sepolia));

        assert_eq!(mainnet.0, parse_cairo_short_string(&NamedChainId::Mainnet.id()).unwrap());
        assert_eq!(goerli.0, parse_cairo_short_string(&NamedChainId::Goerli.id()).unwrap());
        assert_eq!(sepolia.0, parse_cairo_short_string(&NamedChainId::Sepolia.id()).unwrap());
    }
}

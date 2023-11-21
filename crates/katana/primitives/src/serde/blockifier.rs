use std::collections::HashMap;
use std::sync::Arc;

use blockifier::execution::contract_class::{
    ContractClass, ContractClassV0, ContractClassV0Inner, ContractClassV1, ContractClassV1Inner,
};
use cairo_vm::felt::Felt252;
use cairo_vm::hint_processor::hint_processor_definition::HintReference;
use cairo_vm::serde::deserialize_program::{
    ApTracking, Attribute, BuiltinName, FlowTrackingData, HintParams, Identifier,
    InstructionLocation, Member, OffsetValue,
};
use cairo_vm::types::program::{Program, SharedProgramData};
use cairo_vm::types::relocatable::MaybeRelocatable;
use serde::{Deserialize, Serialize};
use starknet_api::core::EntryPointSelector;
use starknet_api::deprecated_contract_class::{EntryPoint, EntryPointOffset, EntryPointType};

#[derive(Debug, Serialize, Deserialize)]
pub enum SerializableContractClass {
    V0(SerializableContractClassV0),
    V1(SerializableContractClassV1),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerializableContractClassV0 {
    pub program: SerializableProgram,
    pub entry_points_by_type: HashMap<EntryPointType, Vec<SerializableEntryPoint>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SerializableContractClassV1 {
    pub program: SerializableProgram,
    pub hints: HashMap<String, Vec<u8>>,
    pub entry_points_by_type: HashMap<EntryPointType, Vec<SerializableEntryPointV1>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableEntryPoint {
    pub selector: EntryPointSelector,
    pub offset: SerializableEntryPointOffset,
}

impl From<EntryPoint> for SerializableEntryPoint {
    fn from(value: EntryPoint) -> Self {
        Self { selector: value.selector, offset: value.offset.into() }
    }
}

impl From<SerializableEntryPoint> for EntryPoint {
    fn from(value: SerializableEntryPoint) -> Self {
        Self { selector: value.selector, offset: value.offset.into() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableEntryPointOffset(pub usize);

impl From<EntryPointOffset> for SerializableEntryPointOffset {
    fn from(value: EntryPointOffset) -> Self {
        Self(value.0)
    }
}

impl From<SerializableEntryPointOffset> for EntryPointOffset {
    fn from(value: SerializableEntryPointOffset) -> Self {
        Self(value.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableEntryPointV1 {
    pub selector: EntryPointSelector,
    pub offset: SerializableEntryPointOffset,
    pub builtins: Vec<String>,
}

impl From<SerializableEntryPointV1> for blockifier::execution::contract_class::EntryPointV1 {
    fn from(value: SerializableEntryPointV1) -> Self {
        blockifier::execution::contract_class::EntryPointV1 {
            selector: value.selector,
            offset: value.offset.into(),
            builtins: value.builtins,
        }
    }
}

impl From<blockifier::execution::contract_class::EntryPointV1> for SerializableEntryPointV1 {
    fn from(value: blockifier::execution::contract_class::EntryPointV1) -> Self {
        SerializableEntryPointV1 {
            selector: value.selector,
            offset: value.offset.into(),
            builtins: value.builtins,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerializableProgram {
    pub shared_program_data: SerializableSharedProgramData,
    pub constants: HashMap<String, Felt252>,
    pub builtins: Vec<BuiltinName>,
}

impl From<Program> for SerializableProgram {
    fn from(value: Program) -> Self {
        Self {
            shared_program_data: value.shared_program_data.as_ref().clone().into(),
            constants: value.constants,
            builtins: value.builtins,
        }
    }
}

impl From<SerializableProgram> for Program {
    fn from(value: SerializableProgram) -> Self {
        Self {
            shared_program_data: Arc::new(value.shared_program_data.into()),
            constants: value.constants,
            builtins: value.builtins,
        }
    }
}

// Fields of `SerializableProgramData` must not rely on `deserialize_any`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerializableSharedProgramData {
    pub data: Vec<MaybeRelocatable>,
    pub hints: HashMap<usize, Vec<SerializableHintParams>>,
    pub main: Option<usize>,
    pub start: Option<usize>,
    pub end: Option<usize>,
    pub error_message_attributes: Vec<SerializableAttribute>,
    pub instruction_locations: Option<HashMap<usize, InstructionLocation>>,
    pub identifiers: HashMap<String, SerializableIdentifier>,
    pub reference_manager: Vec<SerializableHintReference>,
}

impl From<SharedProgramData> for SerializableSharedProgramData {
    fn from(value: SharedProgramData) -> Self {
        Self {
            data: value.data,
            hints: value
                .hints
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().map(|h| h.into()).collect()))
                .collect(),
            main: value.main,
            start: value.start,
            end: value.end,
            error_message_attributes: value
                .error_message_attributes
                .into_iter()
                .map(|a| a.into())
                .collect(),
            instruction_locations: value.instruction_locations,
            identifiers: value.identifiers.into_iter().map(|(k, v)| (k, v.into())).collect(),
            reference_manager: value.reference_manager.into_iter().map(|r| r.into()).collect(),
        }
    }
}

impl From<SerializableSharedProgramData> for SharedProgramData {
    fn from(value: SerializableSharedProgramData) -> Self {
        Self {
            data: value.data,
            hints: value
                .hints
                .into_iter()
                .map(|(k, v)| (k, v.into_iter().map(|h| h.into()).collect()))
                .collect(),
            main: value.main,
            start: value.start,
            end: value.end,
            error_message_attributes: value
                .error_message_attributes
                .into_iter()
                .map(|a| a.into())
                .collect(),
            instruction_locations: value.instruction_locations,
            identifiers: value.identifiers.into_iter().map(|(k, v)| (k, v.into())).collect(),
            reference_manager: value.reference_manager.into_iter().map(|r| r.into()).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerializableHintParams {
    pub code: String,
    pub accessible_scopes: Vec<String>,
    pub flow_tracking_data: SerializableFlowTrackingData,
}

impl From<HintParams> for SerializableHintParams {
    fn from(value: HintParams) -> Self {
        Self {
            code: value.code,
            accessible_scopes: value.accessible_scopes,
            flow_tracking_data: value.flow_tracking_data.into(),
        }
    }
}

impl From<SerializableHintParams> for HintParams {
    fn from(value: SerializableHintParams) -> Self {
        Self {
            code: value.code,
            accessible_scopes: value.accessible_scopes,
            flow_tracking_data: value.flow_tracking_data.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerializableIdentifier {
    pub pc: Option<usize>,
    pub type_: Option<String>,
    pub value: Option<Felt252>,
    pub full_name: Option<String>,
    pub members: Option<HashMap<String, Member>>,
    pub cairo_type: Option<String>,
}

impl From<Identifier> for SerializableIdentifier {
    fn from(value: Identifier) -> Self {
        Self {
            pc: value.pc,
            type_: value.type_,
            value: value.value,
            full_name: value.full_name,
            members: value.members,
            cairo_type: value.cairo_type,
        }
    }
}

impl From<SerializableIdentifier> for Identifier {
    fn from(value: SerializableIdentifier) -> Self {
        Self {
            pc: value.pc,
            type_: value.type_,
            value: value.value,
            full_name: value.full_name,
            members: value.members,
            cairo_type: value.cairo_type,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerializableHintReference {
    pub offset1: OffsetValue,
    pub offset2: OffsetValue,
    pub dereference: bool,
    pub ap_tracking_data: Option<ApTracking>,
    pub cairo_type: Option<String>,
}

impl From<HintReference> for SerializableHintReference {
    fn from(value: HintReference) -> Self {
        Self {
            offset1: value.offset1,
            offset2: value.offset2,
            dereference: value.dereference,
            ap_tracking_data: value.ap_tracking_data,
            cairo_type: value.cairo_type,
        }
    }
}

impl From<SerializableHintReference> for HintReference {
    fn from(value: SerializableHintReference) -> Self {
        Self {
            offset1: value.offset1,
            offset2: value.offset2,
            dereference: value.dereference,
            ap_tracking_data: value.ap_tracking_data,
            cairo_type: value.cairo_type,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerializableAttribute {
    pub name: String,
    pub start_pc: usize,
    pub end_pc: usize,
    pub value: String,
    pub flow_tracking_data: Option<SerializableFlowTrackingData>,
}

impl From<Attribute> for SerializableAttribute {
    fn from(value: Attribute) -> Self {
        Self {
            name: value.name,
            start_pc: value.start_pc,
            end_pc: value.end_pc,
            value: value.value,
            flow_tracking_data: value.flow_tracking_data.map(|d| d.into()),
        }
    }
}

impl From<SerializableAttribute> for Attribute {
    fn from(value: SerializableAttribute) -> Self {
        Self {
            name: value.name,
            start_pc: value.start_pc,
            end_pc: value.end_pc,
            value: value.value,
            flow_tracking_data: value.flow_tracking_data.map(|d| d.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerializableFlowTrackingData {
    pub ap_tracking: ApTracking,
    pub reference_ids: HashMap<String, usize>,
}

impl From<FlowTrackingData> for SerializableFlowTrackingData {
    fn from(value: FlowTrackingData) -> Self {
        Self { ap_tracking: value.ap_tracking, reference_ids: value.reference_ids }
    }
}

impl From<SerializableFlowTrackingData> for FlowTrackingData {
    fn from(value: SerializableFlowTrackingData) -> Self {
        Self { ap_tracking: value.ap_tracking, reference_ids: value.reference_ids }
    }
}

impl From<SerializableContractClass> for blockifier::execution::contract_class::ContractClass {
    fn from(value: SerializableContractClass) -> Self {
        match value {
            SerializableContractClass::V0(v0) => {
                ContractClass::V0(ContractClassV0(Arc::new(ContractClassV0Inner {
                    program: v0.program.into(),
                    entry_points_by_type: v0
                        .entry_points_by_type
                        .into_iter()
                        .map(|(k, v)| (k, v.into_iter().map(|h| h.into()).collect()))
                        .collect(),
                })))
            }
            SerializableContractClass::V1(v1) => {
                ContractClass::V1(ContractClassV1(Arc::new(ContractClassV1Inner {
                    hints: v1
                        .hints
                        .clone()
                        .into_iter()
                        .map(|(k, v)| (k, serde_json::from_slice(&v).expect("valid hint")))
                        .collect(),
                    program: v1.program.into(),
                    entry_points_by_type: v1
                        .entry_points_by_type
                        .into_iter()
                        .map(|(k, v)| {
                            (
                                        k,
                                        v.into_iter()
                                            .map(Into::into)
                                            .collect::<Vec<
                                                blockifier::execution::contract_class::EntryPointV1,
                                            >>(),
                                    )
                        })
                        .collect::<HashMap<_, _>>(),
                })))
            }
        }
    }
}

impl From<blockifier::execution::contract_class::ContractClass> for SerializableContractClass {
    fn from(value: ContractClass) -> Self {
        match value {
            ContractClass::V0(v0) => SerializableContractClass::V0(SerializableContractClassV0 {
                program: v0.program.clone().into(),
                entry_points_by_type: v0
                    .entry_points_by_type
                    .clone()
                    .into_iter()
                    .map(|(k, v)| (k, v.into_iter().map(|h| h.into()).collect()))
                    .collect(),
            }),

            ContractClass::V1(v1) => SerializableContractClass::V1(SerializableContractClassV1 {
                program: v1.program.clone().into(),
                entry_points_by_type: v1
                    .entry_points_by_type
                    .clone()
                    .into_iter()
                    .map(|(k, v)| {
                        (
                            k,
                            v.into_iter()
                                .map(Into::into)
                                .collect::<Vec<SerializableEntryPointV1>>(),
                        )
                    })
                    .collect::<HashMap<_, _>>(),
                hints: v1
                    .hints
                    .clone()
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::to_vec(&v).expect("valid hint")))
                    .collect(),
            }),
        }
    }
}

use std::{collections::HashMap, sync::Arc};

use anyhow::Context;
use cairo_lang_casm::hints::Hint;
use cairo_vm::serde::deserialize_program::{parse_program_json, BuiltinName, ProgramJson};
use serde::{Deserialize, Serialize};
use starknet::core::types::FieldElement;
use starknet_api::{
    core::EntryPointSelector,
    deprecated_contract_class::{EntryPoint, EntryPointOffset, EntryPointType},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerializableContractClass {
    V0(SerializableContractClassV0),
    V1(SerializableContractClassV1),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableContractClassV0 {
    pub program: ProgramJson,
    pub entry_points_by_type: HashMap<EntryPointType, Vec<EntryPoint>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableContractClassV1 {
    pub program: ProgramJson,
    pub entry_points_by_type: HashMap<EntryPointType, Vec<SerializableEntryPointV1>>,
    pub hints: HashMap<String, Hint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableEntryPointV1 {
    pub selector: EntryPointSelector,
    pub offset: EntryPointOffset,
    pub builtins: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableProgram {
    pub shared_program_data: ProgramJson,
    pub constants: HashMap<String, FieldElement>,
    pub builtins: Vec<BuiltinName>,
}

impl From<SerializableEntryPointV1> for blockifier::execution::contract_class::EntryPointV1 {
    fn from(value: SerializableEntryPointV1) -> Self {
        blockifier::execution::contract_class::EntryPointV1 {
            selector: value.selector,
            offset: value.offset,
            builtins: value.builtins,
        }
    }
}

impl From<blockifier::execution::contract_class::EntryPointV1> for SerializableEntryPointV1 {
    fn from(value: blockifier::execution::contract_class::EntryPointV1) -> Self {
        SerializableEntryPointV1 {
            selector: value.selector,
            offset: value.offset,
            builtins: value.builtins,
        }
    }
}

impl TryFrom<SerializableContractClass> for blockifier::execution::contract_class::ContractClass {
    type Error = anyhow::Error;

    fn try_from(value: SerializableContractClass) -> Result<Self, Self::Error> {
        Ok(match value {
            SerializableContractClass::V0(v0) => {
                blockifier::execution::contract_class::ContractClass::V0(
                    blockifier::execution::contract_class::ContractClassV0(Arc::new(
                        blockifier::execution::contract_class::ContractClassV0Inner {
                            program: parse_program_json(
                                v0.program,
                                Some(
                                    &serde_json::to_string(&v0.entry_points_by_type)
                                        .with_context(|| "unable to serialize entry points")?,
                                ),
                            )?,
                            entry_points_by_type: v0.entry_points_by_type,
                        },
                    )),
                )
            }
            SerializableContractClass::V1(v1) => {
                blockifier::execution::contract_class::ContractClass::V1(
                    blockifier::execution::contract_class::ContractClassV1(Arc::new(
                        blockifier::execution::contract_class::ContractClassV1Inner {
                            hints: v1.hints.clone(),
                            program: parse_program_json(
                                v1.program,
                                Some(
                                    &serde_json::to_string(&v1.entry_points_by_type)
                                        .with_context(|| "unable to serialize entry points")?,
                                ),
                            )?,
                            entry_points_by_type: v1
                                .entry_points_by_type
                                .clone()
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
                        },
                    )),
                )
            }
        })
    }
}

impl From<blockifier::execution::contract_class::ContractClass> for SerializableContractClass {
    fn from(value: blockifier::execution::contract_class::ContractClass) -> Self {
        match value {
            blockifier::execution::contract_class::ContractClass::V0(v0) => {
                SerializableContractClass::V0(SerializableContractClassV0 {
                    program: v0.program.clone().into(),
                    entry_points_by_type: v0.entry_points_by_type.clone(),
                })
            }
            blockifier::execution::contract_class::ContractClass::V1(v1) => {
                SerializableContractClass::V1(SerializableContractClassV1 {
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
                    hints: v1.hints.clone(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {}

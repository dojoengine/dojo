use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use starknet_api::core::EntryPointSelector;
use starknet_api::deprecated_contract_class::{EntryPoint, EntryPointOffset, EntryPointType};

use super::program::SerializableProgram;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SerializableContractClass {
    V0(SerializableContractClassV0),
    V1(SerializableContractClassV1),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableContractClassV0 {
    pub program: SerializableProgram,
    pub entry_points_by_type: HashMap<EntryPointType, Vec<SerializableEntryPoint>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableContractClassV1 {
    pub program: SerializableProgram,
    pub entry_points_by_type: HashMap<EntryPointType, Vec<SerializableEntryPointV1>>,
    pub hints: HashMap<String, Vec<u8>>,
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

impl TryFrom<SerializableContractClass> for blockifier::execution::contract_class::ContractClass {
    type Error = anyhow::Error;

    fn try_from(value: SerializableContractClass) -> Result<Self, Self::Error> {
        Ok(match value {
            SerializableContractClass::V0(v0) => {
                blockifier::execution::contract_class::ContractClass::V0(
                    blockifier::execution::contract_class::ContractClassV0(Arc::new(
                        blockifier::execution::contract_class::ContractClassV0Inner {
                            program: v0.program.into(),
                            entry_points_by_type: v0
                                .entry_points_by_type
                                .into_iter()
                                .map(|(k, v)| (k, v.into_iter().map(|h| h.into()).collect()))
                                .collect(),
                        },
                    )),
                )
            }
            SerializableContractClass::V1(v1) => {
                blockifier::execution::contract_class::ContractClass::V1(
                    blockifier::execution::contract_class::ContractClassV1(Arc::new(
                        blockifier::execution::contract_class::ContractClassV1Inner {
                            hints: v1
                                .hints
                                .clone()
                                .into_iter()
                                .map(|(k, v)| (k, serde_json::from_slice(&v).unwrap()))
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
                    entry_points_by_type: v0
                        .entry_points_by_type
                        .clone()
                        .into_iter()
                        .map(|(k, v)| (k, v.into_iter().map(|h| h.into()).collect()))
                        .collect(),
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
                    hints: v1
                        .hints
                        .clone()
                        .into_iter()
                        .map(|(k, v)| (k, serde_json::to_vec(&v).unwrap()))
                        .collect(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use blockifier::execution::contract_class::ContractClass;
    use starknet::core::types::contract::SierraClass;

    use super::*;
    use crate::constants::UDC_CONTRACT;
    use crate::utils::contract::rpc_to_inner_class;

    #[test]
    fn serialize_and_deserialize_legacy_contract() {
        let original_contract = UDC_CONTRACT.clone();

        let serializable_contract: SerializableContractClass = original_contract.clone().into();
        assert!(matches!(serializable_contract, SerializableContractClass::V0(_)));

        let bytes = serde_json::to_vec(&serializable_contract).unwrap();
        let serializable_contract: SerializableContractClass =
            serde_json::from_slice(&bytes).unwrap();

        let contract: ContractClass = serializable_contract.try_into().expect("should deserialize");
        assert_eq!(contract, original_contract);
    }

    #[test]
    fn serialize_and_deserialize_contract() {
        let class = serde_json::from_str::<SierraClass>(include_str!(
            "../../../contracts/compiled/cairo1_contract.json"
        ))
        .expect("should deserialize sierra class")
        .flatten()
        .expect("should flatten");

        let (_, original_contract) =
            rpc_to_inner_class(&class).expect("should convert from flattened to contract class");

        let serializable_contract: SerializableContractClass = original_contract.clone().into();
        assert!(matches!(serializable_contract, SerializableContractClass::V1(_)));

        let bytes = serde_json::to_vec(&serializable_contract).unwrap();
        let serializable_contract: SerializableContractClass =
            serde_json::from_slice(&bytes).unwrap();

        let contract: ContractClass = serializable_contract.try_into().expect("should deserialize");
        assert_eq!(contract, original_contract);
    }
}

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use starknet_api::deprecated_contract_class::EntryPointType;

use crate::serde::blockifier::{
    SerializableEntryPoint, SerializableEntryPointV1, SerializableProgram,
};

/// Storeable version of the [`ContractClass`](blockifier::execution::contract_class::ContractClass)
/// type from `blockifier`.
#[derive(Debug, Serialize, Deserialize)]
pub enum StoredCompiledContractClass {
    V0(StoredCompiledContractClassV0),
    V1(StoredCompiledContractClassV1),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredCompiledContractClassV0 {
    pub program: SerializableProgram,
    pub entry_points_by_type: HashMap<EntryPointType, Vec<SerializableEntryPoint>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StoredCompiledContractClassV1 {
    pub program: SerializableProgram,
    pub hints: HashMap<String, Vec<u8>>,
    pub entry_points_by_type: HashMap<EntryPointType, Vec<SerializableEntryPointV1>>,
}

impl TryFrom<StoredCompiledContractClass> for blockifier::execution::contract_class::ContractClass {
    type Error = anyhow::Error;

    fn try_from(value: StoredCompiledContractClass) -> Result<Self, Self::Error> {
        Ok(match value {
            StoredCompiledContractClass::V0(v0) => {
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
            StoredCompiledContractClass::V1(v1) => {
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

impl From<blockifier::execution::contract_class::ContractClass> for StoredCompiledContractClass {
    fn from(value: blockifier::execution::contract_class::ContractClass) -> Self {
        match value {
            blockifier::execution::contract_class::ContractClass::V0(v0) => {
                StoredCompiledContractClass::V0(StoredCompiledContractClassV0 {
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
                StoredCompiledContractClass::V1(StoredCompiledContractClassV1 {
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

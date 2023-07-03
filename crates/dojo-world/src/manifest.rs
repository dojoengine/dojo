use std::fs;
use std::path::Path;

use ::serde::{Deserialize, Serialize};
use anyhow::{anyhow, Result};
use dojo_types::component::Member;
use dojo_types::system::Dependency;
use serde_with::serde_as;
use smol_str::SmolStr;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::{BlockId, BlockTag, FieldElement, FunctionCall, StarknetError};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_selector_from_name, CairoShortStringToFeltError,
};
use starknet::providers::{Provider, ProviderError};
use thiserror::Error;

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

pub const WORLD_CONTRACT_NAME: &str = "world";
pub const EXECUTOR_CONTRACT_NAME: &str = "executor";

#[derive(Error, Debug)]
pub enum ManifestError<E> {
    #[error("Remote World not found.")]
    RemoteWorldNotFound,
    #[error("Executor contract not found.")]
    ExecutorNotFound,
    #[error("Entry point name contains non-ASCII characters.")]
    InvalidEntryPointError,
    #[error(transparent)]
    InvalidNameError(CairoShortStringToFeltError),
    #[error(transparent)]
    Provider(ProviderError<E>),
}

/// Represents a declaration of a component.
#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Component {
    pub name: String,
    pub members: Vec<Member>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
}

/// System input ABI.
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Input {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// System Output ABI.
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Output {
    #[serde(rename = "type")]
    pub ty: String,
}

/// Represents a declaration of a system.
#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct System {
    pub name: SmolStr,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub dependencies: Vec<Dependency>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Contract {
    pub name: SmolStr,
    #[serde_as(as = "Option<UfeHex>")]
    pub address: Option<FieldElement>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub world: Contract,
    pub executor: Contract,
    pub systems: Vec<System>,
    pub contracts: Vec<Contract>,
    pub components: Vec<Component>,
}

impl Manifest {
    pub fn load_from_path<P>(manifest_path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        serde_json::from_reader(fs::File::open(manifest_path)?)
            .map_err(|e| anyhow!("Failed to load World manifest from path: {e}"))
    }

    pub async fn from_remote<P>(
        provider: P,
        world_address: FieldElement,
        match_manifest: Option<Manifest>,
    ) -> Result<Self, ManifestError<<P as Provider>::Error>>
    where
        P: Provider + Send,
    {
        let world_class_hash = provider
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), world_address)
            .await
            .map_err(|err| match err {
                ProviderError::StarknetError(StarknetError::ContractNotFound) => {
                    ManifestError::RemoteWorldNotFound
                }
                _ => ManifestError::Provider(err),
            })?;

        let executor_address = provider
            .call(
                FunctionCall {
                    contract_address: world_address,
                    calldata: vec![],
                    entry_point_selector: get_selector_from_name("executor").unwrap(),
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await
            .map_err(ManifestError::Provider)?[0];

        let executor_class_hash = provider
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), executor_address)
            .await
            .map_err(|err| match err {
                ProviderError::StarknetError(StarknetError::ContractNotFound) => {
                    ManifestError::ExecutorNotFound
                }
                _ => ManifestError::Provider(err),
            })?;

        let mut systems = vec![];
        let mut components = vec![];

        if let Some(match_manifest) = match_manifest {
            for component in match_manifest.components {
                let result = provider
                    .call(
                        FunctionCall {
                            contract_address: world_address,
                            calldata: vec![
                                cairo_short_string_to_felt(&component.name)
                                    .map_err(ManifestError::InvalidNameError)?,
                            ],
                            entry_point_selector: get_selector_from_name("component").unwrap(),
                        },
                        BlockId::Tag(BlockTag::Pending),
                    )
                    .await
                    .map_err(ManifestError::Provider)?;

                components.push(Component {
                    name: component.name.clone(),
                    class_hash: result[0],
                    ..Default::default()
                });
            }

            for system in match_manifest.systems {
                let result = provider
                    .call(
                        FunctionCall {
                            contract_address: world_address,
                            calldata: vec![
                                cairo_short_string_to_felt(
                                    // because the name returns by the `name` method of
                                    // a system contract is without the 'System' suffix
                                    system.name.strip_suffix("System").unwrap_or(&system.name),
                                )
                                .map_err(ManifestError::InvalidNameError)?,
                            ],
                            entry_point_selector: get_selector_from_name("system").unwrap(),
                        },
                        BlockId::Tag(BlockTag::Pending),
                    )
                    .await
                    .map_err(ManifestError::Provider)?;

                systems.push(System {
                    name: system.name.clone(),
                    class_hash: result[0],
                    ..Default::default()
                });
            }
        }

        Ok(Manifest {
            systems,
            components,
            contracts: vec![],
            world: Contract {
                name: WORLD_CONTRACT_NAME.into(),
                class_hash: world_class_hash,
                address: Some(world_address),
            },
            executor: Contract {
                name: EXECUTOR_CONTRACT_NAME.into(),
                address: Some(executor_address),
                class_hash: executor_class_hash,
            },
        })
    }
}

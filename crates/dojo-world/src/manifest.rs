use std::fs;
use std::path::Path;

use ::serde::{Deserialize, Serialize};
use anyhow::{anyhow, Result};
use serde_with::serde_as;
use smol_str::SmolStr;
use starknet::core::serde::unsigned_field_element::{UfeHex, UfeHexOption};
use starknet::core::types::{CallContractResult, CallFunction, FieldElement};
use starknet::core::utils::{cairo_short_string_to_felt, CairoShortStringToFeltError};
use starknet::providers::jsonrpc::models::{BlockId, BlockTag, ErrorCode};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcClientError, JsonRpcTransport, RpcError};
use starknet::providers::Provider;
use thiserror::Error;

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

const EXECUTOR_ADDRESS_SLOT: FieldElement = FieldElement::from_mont([
    7467091854009816808,
    5217539096067869628,
    17301706476858600182,
    440859966107478631,
]);

const COMPONENT_ENTRYPOINT: FieldElement = FieldElement::from_mont([
    2012748018737461584,
    17346441013657197760,
    13481606495872588402,
    416862702099901043,
]);

const SYSTEM_ENTRYPOINT: FieldElement = FieldElement::from_mont([
    5274299164659238291,
    8011946809036665273,
    17510334645946118431,
    553330538481721971,
]);

#[derive(Error, Debug)]
pub enum ManifestError<T> {
    #[error(transparent)]
    ClientError(JsonRpcClientError<T>),
    #[error("World not deployed.")]
    NotDeployed,
    #[error("Entry point name contains non-ASCII characters.")]
    InvalidEntryPointError,
    #[error(transparent)]
    InvalidNameError(CairoShortStringToFeltError),
    #[error("Provider error.")]
    ProviderError,
}

/// Component member.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// Represents a declaration of a component.
#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    pub members: Vec<Member>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
}

/// System input ABI.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Input {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// System Output ABI.
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Output {
    #[serde(rename = "type")]
    pub ty: String,
}

/// Represents a declaration of a system.
#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct System {
    pub name: SmolStr,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub dependencies: Vec<String>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Contract {
    pub name: SmolStr,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Manifest {
    #[serde_as(as = "UfeHexOption")]
    pub world: Option<FieldElement>,
    #[serde_as(as = "UfeHexOption")]
    pub executor: Option<FieldElement>,
    pub components: Vec<Component>,
    pub systems: Vec<System>,
    pub contracts: Vec<Contract>,
}

impl Manifest {
    pub fn load_from_path<P>(manifest_path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        serde_json::from_reader(fs::File::open(manifest_path)?)
            .map_err(|e| anyhow!("Problem in loading manifest from path: {e}"))
    }

    pub async fn from_remote<P: JsonRpcTransport + Sync + Send>(
        world_address: FieldElement,
        provider: JsonRpcClient<P>,
        match_manifest: Option<Manifest>,
    ) -> Result<Self, ManifestError<P::Error>> {
        let mut manifest = Manifest::default();

        let world_class_hash = provider
            .get_class_hash_at(&BlockId::Tag(BlockTag::Pending), world_address)
            .await
            .map_err(|err| match err {
                JsonRpcClientError::RpcError(RpcError::Code(ErrorCode::ContractNotFound)) => {
                    ManifestError::NotDeployed
                }
                _ => ManifestError::ClientError(err),
            })?;

        let executor_address = provider
            .get_storage_at(world_address, EXECUTOR_ADDRESS_SLOT, &BlockId::Tag(BlockTag::Pending))
            .await
            .map_err(ManifestError::ClientError)?;

        let executor_class_hash = provider
            .get_class_hash_at(&BlockId::Tag(BlockTag::Pending), executor_address)
            .await
            .ok();

        manifest.world = Some(world_class_hash);
        manifest.executor = executor_class_hash;

        if let Some(match_manifest) = match_manifest {
            for component in match_manifest.components {
                let CallContractResult { result } = provider
                    .call_contract(
                        CallFunction {
                            contract_address: world_address,
                            calldata: vec![
                                cairo_short_string_to_felt(&component.name)
                                    .map_err(ManifestError::InvalidNameError)?,
                            ],
                            entry_point_selector: COMPONENT_ENTRYPOINT,
                        },
                        starknet::core::types::BlockId::Pending,
                    )
                    .await
                    .map_err(|_| ManifestError::ProviderError)?;

                manifest.components.push(Component {
                    name: component.name.clone(),
                    class_hash: result[0],
                    ..Default::default()
                });
            }

            for system in match_manifest.systems {
                let CallContractResult { result } = provider
                    .call_contract(
                        CallFunction {
                            contract_address: world_address,
                            calldata: vec![
                                cairo_short_string_to_felt(
                                    // because the name returns by the `name` method of
                                    // a system contract is without the 'System' suffix
                                    system.name.strip_suffix("System").unwrap_or(&system.name),
                                )
                                .map_err(ManifestError::InvalidNameError)?,
                            ],
                            entry_point_selector: SYSTEM_ENTRYPOINT,
                        },
                        starknet::core::types::BlockId::Pending,
                    )
                    .await
                    .map_err(|_| ManifestError::ProviderError)?;

                manifest.systems.push(System {
                    name: system.name.clone(),
                    class_hash: result[0],
                    ..Default::default()
                });
            }
        }

        Ok(manifest)
    }
}

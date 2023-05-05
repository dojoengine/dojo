use std::fs;
use std::path::Path;

use ::serde::{Deserialize, Serialize};
use anyhow::{anyhow, Result};
use serde_with::serde_as;
use smol_str::SmolStr;
use starknet::core::serde::unsigned_field_element::{UfeHex, UfeHexOption};
use starknet::core::types::{CallContractResult, CallFunction, FieldElement};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_selector_from_name, get_storage_var_address,
};
use starknet::providers::jsonrpc::models::{BlockId, BlockTag};
use starknet::providers::jsonrpc::{JsonRpcClient, JsonRpcTransport};
use starknet::providers::Provider;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ManifestError {
    #[error("Provider error.")]
    ProviderError,
}

/// Component member.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// Represents a declaration of a component.
#[serde_as]
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    pub members: Vec<Member>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
}

/// System input ABI.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Input {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// System Output ABI.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Output {
    #[serde(rename = "type")]
    pub ty: String,
}

/// Represents a declaration of a system.
#[serde_as]
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct System {
    pub name: SmolStr,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub dependencies: Vec<String>,
}

#[serde_as]
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Contract {
    pub name: SmolStr,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
}

#[serde_as]
#[derive(Default, Debug, Serialize, Deserialize)]
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
        local_manifest: &Self,
    ) -> Result<Self> {
        let mut manifest = Manifest::default();

        let world_class_hash =
            provider.get_class_hash_at(&BlockId::Tag(BlockTag::Pending), world_address).await.ok();

        if world_class_hash.is_none() {
            return Ok(manifest);
        }

        let executor_address = provider
            .get_storage_at(
                world_address,
                get_storage_var_address("executor", &[])?,
                &BlockId::Tag(BlockTag::Pending),
            )
            .await
            .map_err(|_| ManifestError::ProviderError)?;

        let executor_class_hash = provider
            .get_class_hash_at(&BlockId::Tag(BlockTag::Pending), executor_address)
            .await
            .ok();

        manifest.world = world_class_hash;
        manifest.executor = executor_class_hash;

        for component in &local_manifest.components {
            let CallContractResult { result } = provider
                .call_contract(
                    CallFunction {
                        contract_address: world_address,
                        calldata: vec![cairo_short_string_to_felt(&component.name)?],
                        entry_point_selector: get_selector_from_name("component")?,
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

        for system in &local_manifest.systems {
            let CallContractResult { result } = provider
                .call_contract(
                    CallFunction {
                        contract_address: world_address,
                        calldata: vec![cairo_short_string_to_felt(
                            // because the name returns by the `name` method of
                            // a system contract is without the 'System' suffix
                            system.name.strip_suffix("System").unwrap_or(&system.name),
                        )?],
                        entry_point_selector: get_selector_from_name("system")?,
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

        Ok(manifest)
    }
}

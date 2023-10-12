use std::fs;
use std::path::Path;

use ::serde::{Deserialize, Serialize};
use cairo_lang_starknet::abi;
use serde_with::serde_as;
use smol_str::SmolStr;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::{BlockId, BlockTag, FieldElement, FunctionCall, StarknetError};
use starknet::core::utils::{
    cairo_short_string_to_felt, get_selector_from_name, CairoShortStringToFeltError,
};
use starknet::providers::{
    MaybeUnknownErrorCode, Provider, ProviderError, StarknetErrorWithMessage,
};
use thiserror::Error;

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

pub const WORLD_CONTRACT_NAME: &str = "world";
pub const EXECUTOR_CONTRACT_NAME: &str = "executor";
pub const BASE_CONTRACT_NAME: &str = "base";

#[derive(Error, Debug)]
pub enum ManifestError<E> {
    #[error("Remote World not found.")]
    RemoteWorldNotFound,
    #[error("Executor contract not found.")]
    ExecutorNotFound,
    #[error("Entry point name contains non-ASCII characters.")]
    InvalidEntryPointError,
    #[error(transparent)]
    InvalidNameError(#[from] CairoShortStringToFeltError),
    #[error(transparent)]
    Provider(#[from] ProviderError<E>),
}

/// Represents a model member.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Member {
    /// Name of the member.
    pub name: String,
    /// Type of the member.
    #[serde(rename = "type")]
    pub ty: String,
    pub key: bool,
}

impl From<dojo_types::schema::Member> for Member {
    fn from(m: dojo_types::schema::Member) -> Self {
        Self { name: m.name, ty: m.ty.name(), key: m.key }
    }
}

/// Represents a declaration of a model.
#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Model {
    pub name: String,
    pub members: Vec<Member>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<abi::Contract>,
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

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Contract {
    pub name: SmolStr,
    #[serde_as(as = "Option<UfeHex>")]
    pub address: Option<FieldElement>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<abi::Contract>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Class {
    pub name: SmolStr,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub abi: Option<abi::Contract>,
}

#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Manifest {
    pub world: Contract,
    pub executor: Contract,
    pub base: Class,
    pub contracts: Vec<Contract>,
    pub models: Vec<Model>,
}

impl Manifest {
    /// Load the manifest from a file at the given path.
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let file = fs::File::open(path)?;
        Ok(serde_json::from_reader(file)?)
    }

    /// Writes the manifest into a file at the given path. Will return error if the file doesn't
    /// exist.
    pub fn write_to_path(self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        let fd = fs::File::options().write(true).open(path)?;
        Ok(serde_json::to_writer_pretty(fd, &self)?)
    }

    /// Construct a manifest of a remote World.
    ///
    /// # Arguments
    /// * `provider` - A Starknet RPC provider.
    /// * `world_address` - The address of the remote World contract.
    pub async fn load_from_remote<P>(
        provider: P,
        world_address: FieldElement,
        match_manifest: Option<Manifest>,
    ) -> Result<Self, ManifestError<<P as Provider>::Error>>
    where
        P: Provider + Send + Sync,
    {
        let world_class_hash = provider
            .get_class_hash_at(BlockId::Tag(BlockTag::Pending), world_address)
            .await
            .map_err(|err| match err {
                ProviderError::StarknetError(StarknetErrorWithMessage {
                    code: MaybeUnknownErrorCode::Known(StarknetError::ContractNotFound),
                    ..
                }) => ManifestError::RemoteWorldNotFound,
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
                ProviderError::StarknetError(StarknetErrorWithMessage {
                    code: MaybeUnknownErrorCode::Known(StarknetError::ContractNotFound),
                    ..
                }) => ManifestError::ExecutorNotFound,
                _ => ManifestError::Provider(err),
            })?;

        let base_class_hash = provider
            .call(
                FunctionCall {
                    contract_address: world_address,
                    calldata: vec![],
                    entry_point_selector: get_selector_from_name("base").unwrap(),
                },
                BlockId::Tag(BlockTag::Pending),
            )
            .await
            .map_err(ManifestError::Provider)?[0];

        let mut models = vec![];

        if let Some(match_manifest) = match_manifest {
            for model in match_manifest.models {
                let result = provider
                    .call(
                        FunctionCall {
                            contract_address: world_address,
                            calldata: vec![
                                cairo_short_string_to_felt(&model.name)
                                    .map_err(ManifestError::InvalidNameError)?,
                            ],
                            entry_point_selector: get_selector_from_name("model").unwrap(),
                        },
                        BlockId::Tag(BlockTag::Pending),
                    )
                    .await
                    .map_err(ManifestError::Provider)?;

                models.push(Model {
                    name: model.name.clone(),
                    class_hash: result[0],
                    ..Default::default()
                });
            }
        }

        Ok(Manifest {
            models,
            contracts: vec![],
            world: Contract {
                name: WORLD_CONTRACT_NAME.into(),
                class_hash: world_class_hash,
                address: Some(world_address),
                ..Default::default()
            },
            executor: Contract {
                name: EXECUTOR_CONTRACT_NAME.into(),
                address: Some(executor_address),
                class_hash: executor_class_hash,
                ..Default::default()
            },
            base: Class {
                name: BASE_CONTRACT_NAME.into(),
                class_hash: base_class_hash,
                ..Default::default()
            },
        })
    }
}

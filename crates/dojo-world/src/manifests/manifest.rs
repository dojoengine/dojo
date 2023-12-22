use std::collections::HashMap;
use std::fs;
use std::path::Path;

use ::serde::{Deserialize, Serialize};
use cainome::cairo_serde::Error as CainomeError;
use cairo_lang_starknet::abi;
use serde_with::serde_as;
use smol_str::SmolStr;
use starknet::core::serde::unsigned_field_element::UfeHex;
use starknet::core::types::{
    BlockId, BlockTag, EmittedEvent, EventFilter, FieldElement, FunctionCall, StarknetError,
};
use starknet::core::utils::{
    parse_cairo_short_string, starknet_keccak, CairoShortStringToFeltError,
    ParseCairoShortStringError,
};
use starknet::macros::selector;
use starknet::providers::{Provider, ProviderError};
use thiserror::Error;

use crate::contracts::model::ModelError;
use crate::contracts::WorldContractReader;

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

pub const WORLD_CONTRACT_NAME: &str = "dojo::world::world";
pub const EXECUTOR_CONTRACT_NAME: &str = "dojo::executor::executor";
pub const BASE_CONTRACT_NAME: &str = "dojo::base::base";

#[derive(Error, Debug)]
pub enum WorldError {
    #[error("Remote World not found.")]
    RemoteWorldNotFound,
    #[error("Executor contract not found.")]
    ExecutorNotFound,
    #[error("Entry point name contains non-ASCII characters.")]
    InvalidEntryPointError,
    #[error(transparent)]
    CairoShortStringToFelt(#[from] CairoShortStringToFeltError),
    #[error(transparent)]
    ParseCairoShortString(#[from] ParseCairoShortStringError),
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error(transparent)]
    ContractRead(#[from] CainomeError),
    #[error(transparent)]
    Model(#[from] ModelError),
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
pub struct ComputedValueEntrypoint {
    // Name of the contract containing the entrypoint
    pub contract: SmolStr,
    // Name of entrypoint to get computed value
    pub entrypoint: SmolStr,
    // Component to compute for
    pub model: Option<String>,
}

#[serde_as]
#[derive(Clone, Default, Debug, Serialize, Deserialize, PartialEq)]
pub struct Contract {
    // pub name: SmolStr,
    #[serde_as(as = "UfeHex")]
    pub address: FieldElement
    // #[serde_as(as = "UfeHex")]
    // pub class_hash: FieldElement,
    // pub abi: Option<abi::Contract>,
    // pub reads: Vec<String>,
    // pub writes: Vec<String>,
    // pub computed: Vec<ComputedValueEntrypoint>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum ManifestKind {
    Class,
    Contract(Contract),
}

#[serde_as]
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct Manifest {
    pub kind: ManifestKind,
    pub name: SmolStr,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct World {
    pub world: Manifest,
    pub executor: Manifest,
    pub base: Manifest,
    pub contracts: Vec<Manifest>,
    pub models: Vec<Manifest>,
}

impl World {
    /// Load the manifest from a file at the given path.
    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let file = fs::File::open(path)?;
        Ok(Self::try_from(file)?)
    }

    /// Writes the manifest into a file at the given path. Will return error if the file doesn't
    /// exist.
    pub fn write_to_path(self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        let fd = fs::File::options().write(true).open(path)?;
        Ok(serde_json::to_writer_pretty(fd, &self)?)
    }
}

impl TryFrom<std::fs::File> for World {
    type Error = serde_json::Error;
    fn try_from(file: std::fs::File) -> Result<Self, Self::Error> {
        serde_json::from_reader(std::io::BufReader::new(file))
    }
}

impl TryFrom<&std::fs::File> for World {
    type Error = serde_json::Error;
    fn try_from(file: &std::fs::File) -> Result<Self, Self::Error> {
        serde_json::from_reader(std::io::BufReader::new(file))
    }
}

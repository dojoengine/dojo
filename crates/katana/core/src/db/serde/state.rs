use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use ::serde::{Deserialize, Serialize};
use flate2::read::GzDecoder;
use starknet::core::types::{FieldElement, FlattenedSierraClass};

use crate::db::serde::contract::SerializableContractClass;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableState {
    /// Address to storage record.
    pub storage: BTreeMap<FieldElement, SerializableStorageRecord>,
    /// Class hash to class record.
    pub classes: BTreeMap<FieldElement, SerializableClassRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableClassRecord {
    pub compiled_hash: FieldElement,
    pub class: SerializableContractClass,
    pub sierra_class: Option<FlattenedSierraClass>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableStorageRecord {
    pub nonce: FieldElement,
    pub class_hash: FieldElement,
    pub storage: BTreeMap<FieldElement, FieldElement>,
}

impl SerializableState {
    /// Loads the serialized state from the given path
    pub fn load(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        let path = path.as_ref();
        let buf = if path.is_dir() { fs::read(path.join("state.bin"))? } else { fs::read(path)? };

        let mut decoder = GzDecoder::new(&buf[..]);
        let mut decoded_data: Vec<u8> = Vec::new();

        let state = serde_json::from_slice(if decoder.header().is_some() {
            decoder.read_to_end(decoded_data.as_mut())?;
            &decoded_data
        } else {
            &buf
        })?;

        Ok(state)
    }

    /// This is used as the clap `value_parser` implementation
    pub fn parse(path: &str) -> Result<Self, String> {
        Self::load(path).map_err(|err| err.to_string())
    }
}

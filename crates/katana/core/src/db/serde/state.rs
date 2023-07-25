use std::collections::BTreeMap;

use crate::db::serde::contract::SerializableContractClass;
use ::serde::{Deserialize, Serialize};
use starknet::core::types::{FieldElement, FlattenedSierraClass};

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

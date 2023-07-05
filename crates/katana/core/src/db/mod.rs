pub mod contract;

use std::collections::BTreeMap;

use anyhow::Result;
use blockifier::state::state_api::{State, StateReader};
use serde::{Deserialize, Serialize};
use starknet::core::types::{FieldElement, FlattenedSierraClass};
use starknet_api::{
    core::{ClassHash, CompiledClassHash, ContractAddress, Nonce, PatriciaKey},
    hash::StarkHash,
    patricia_key,
    state::StorageKey,
};

use crate::{db::contract::SerializableContractClass, state::StateExt};

pub trait Db: State + StateReader + StateExt {
    fn set_nonce(&mut self, addr: ContractAddress, nonce: Nonce);

    fn dump_state(&self) -> Result<SerializableState>;

    fn load_state(&mut self, state: SerializableState) -> Result<()> {
        for (addr, record) in state.state.iter() {
            let address = ContractAddress(patricia_key!(*addr));

            record.storage.iter().for_each(|(key, value)| {
                self.set_storage_at(address, StorageKey(patricia_key!(*key)), (*value).into());
            });

            self.set_class_hash_at(address, ClassHash(record.class_hash.into()))?;
            self.set_nonce(address, Nonce(record.nonce.into()));
        }

        for (hash, record) in state.classes.iter() {
            let hash = ClassHash((*hash).into());
            let compiled_hash = CompiledClassHash(record.compiled_hash.into());

            self.set_contract_class(&hash, record.class.clone().try_into()?)?;
            self.set_compiled_class_hash(hash, compiled_hash)?;

            if let Some(sierra_class) = record.sierra_class.clone() {
                self.set_sierra_class(hash, sierra_class)?;
            }
        }

        todo!()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableState {
    /// Address to storage record.
    state: BTreeMap<FieldElement, SerializableStorageRecord>,
    /// Class hash to class record.
    classes: BTreeMap<FieldElement, SerializableClassRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableClassRecord {
    compiled_hash: FieldElement,
    class: SerializableContractClass,
    sierra_class: Option<FlattenedSierraClass>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SerializableStorageRecord {
    nonce: FieldElement,
    class_hash: FieldElement,
    storage: BTreeMap<FieldElement, FieldElement>,
}

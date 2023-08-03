pub mod serde;

use anyhow::Result;
use blockifier::state::state_api::{State, StateReader};
use starknet_api::core::{ClassHash, CompiledClassHash, ContractAddress, Nonce, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;
use starknet_api::state::StorageKey;

use self::serde::state::SerializableState;
use crate::backend::state::StateExt;

pub trait Db: State + StateReader + StateExt {
    fn set_nonce(&mut self, addr: ContractAddress, nonce: Nonce);

    fn dump_state(&self) -> Result<SerializableState>;

    fn load_state(&mut self, state: SerializableState) -> Result<()> {
        for (addr, record) in state.storage.iter() {
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

        Ok(())
    }
}

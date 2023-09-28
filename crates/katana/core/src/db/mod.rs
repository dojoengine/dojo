use std::fmt;
use std::sync::Arc;

use anyhow::Result;
use blockifier::execution::contract_class::ContractClass;
use blockifier::state::state_api::{State, StateReader, StateResult};
use parking_lot::Mutex;
use starknet::core::types::FlattenedSierraClass;
use starknet_api::core::{ClassHash, CompiledClassHash, ContractAddress, Nonce, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;
use starknet_api::state::StorageKey;

use self::cached::MaybeAsCachedDb;
use self::serde::state::SerializableState;

pub mod cached;
pub mod serde;

/// An extension of the [StateReader] trait, to allow fetching Sierra class from the state.
pub trait StateExtRef: StateReader + fmt::Debug {
    /// Returns the Sierra class for the given class hash.
    fn get_sierra_class(&mut self, class_hash: &ClassHash) -> StateResult<FlattenedSierraClass>;
}

/// An extension of the [State] trait, to allow storing Sierra classes.
pub trait StateExt: State + StateExtRef {
    /// Set the Sierra class for the given class hash.
    fn set_sierra_class(
        &mut self,
        class_hash: ClassHash,
        sierra_class: FlattenedSierraClass,
    ) -> StateResult<()>;
}

/// A trait which represents a state database.
pub trait Database: StateExt + AsStateRefDb + Send + Sync + MaybeAsCachedDb {
    /// Set the exact nonce value for the given contract address.
    fn set_nonce(&mut self, addr: ContractAddress, nonce: Nonce);

    /// Returns the serialized version of the state.
    fn dump_state(&self) -> Result<SerializableState>;

    /// Load the serialized state into the current state.
    fn load_state(&mut self, state: SerializableState) -> Result<()> {
        for (addr, record) in state.storage {
            let address = ContractAddress(patricia_key!(addr));

            record.storage.iter().for_each(|(key, value)| {
                self.set_storage_at(address, StorageKey(patricia_key!(*key)), (*value).into());
            });

            self.set_nonce(address, Nonce(record.nonce.into()));
        }

        for (address, class_hash) in state.contracts {
            self.set_class_hash_at(
                ContractAddress(patricia_key!(address)),
                ClassHash(class_hash.into()),
            )?;
        }

        for (hash, record) in state.classes {
            let hash = ClassHash(hash.into());
            let compiled_hash = CompiledClassHash(record.compiled_hash.into());

            self.set_contract_class(&hash, record.class.clone().try_into()?)?;
            self.set_compiled_class_hash(hash, compiled_hash)?;
        }

        for (hash, sierra_class) in state.sierra_classes {
            let hash = ClassHash(hash.into());
            self.set_sierra_class(hash, sierra_class.clone())?;
        }

        Ok(())
    }
}

pub trait AsStateRefDb {
    /// Returns the current state as a read only state
    fn as_ref_db(&self) -> StateRefDb;
}

/// A type which represents a state at a cetain point. This state type is only meant to be read
/// from.
///
/// It implements [Clone] so that it can be cloned into a
/// [CachedState](blockifier::state::cached_state::CachedState) for executing transactions
/// based on this state, as [CachedState](blockifier::state::cached_state::CachedState) requires the
/// ownership of the inner [StateReader] that it wraps.
///
/// The inner type is wrapped inside a [Mutex] to allow interior mutability due to the fact
/// that the [StateReader] trait requires mutable access to the type that implements it.
#[derive(Debug, Clone)]
pub struct StateRefDb(Arc<Mutex<dyn StateExtRef + Send + Sync>>);

impl StateRefDb {
    pub fn new<T>(state: T) -> Self
    where
        T: StateExtRef + Send + Sync + 'static,
    {
        Self(Arc::new(Mutex::new(state)))
    }
}

impl StateReader for StateRefDb {
    fn get_storage_at(&mut self, addr: ContractAddress, key: StorageKey) -> StateResult<StarkHash> {
        self.0.lock().get_storage_at(addr, key)
    }

    fn get_class_hash_at(&mut self, addr: ContractAddress) -> StateResult<ClassHash> {
        self.0.lock().get_class_hash_at(addr)
    }

    fn get_compiled_class_hash(&mut self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        self.0.lock().get_compiled_class_hash(class_hash)
    }

    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        self.0.lock().get_nonce_at(contract_address)
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: &ClassHash,
    ) -> StateResult<ContractClass> {
        self.0.lock().get_compiled_contract_class(class_hash)
    }
}

impl StateExtRef for StateRefDb {
    fn get_sierra_class(&mut self, class_hash: &ClassHash) -> StateResult<FlattenedSierraClass> {
        self.0.lock().get_sierra_class(class_hash)
    }
}

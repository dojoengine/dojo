use std::collections::HashMap;

use blockifier::state::cached_state::{CachedState, GlobalContractCache};
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{StateReader, StateResult};
use katana_primitives::contract::{CompiledClass, FlattenedSierraClass};
use katana_primitives::conversion::blockifier::to_class;
use katana_primitives::FieldElement;
use katana_provider::traits::contract::ContractClassProvider;
use katana_provider::traits::state::StateProvider;
use katana_provider::ProviderResult;
use parking_lot::{Mutex, RawMutex, RwLock};
use starknet_api::core::{ClassHash, CompiledClassHash, Nonce, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;
use starknet_api::state::StorageKey;

mod primitives {
    pub use katana_primitives::contract::{
        ClassHash, CompiledClassHash, ContractAddress, Nonce, StorageKey, StorageValue,
    };
}

/// A state db only provide read access.
///
/// This type implements the [`StateReader`] trait so that it can be used as a with [`CachedState`].
pub struct StateRefDb(pub Box<dyn StateProvider>);

impl StateRefDb {
    pub fn new(provider: impl StateProvider + 'static) -> Self {
        Self(Box::new(provider))
    }
}

impl StateProvider for StateRefDb {
    fn class_hash_of_contract(
        &self,
        address: primitives::ContractAddress,
    ) -> ProviderResult<Option<primitives::ClassHash>> {
        self.0.class_hash_of_contract(address)
    }

    fn nonce(
        &self,
        address: primitives::ContractAddress,
    ) -> ProviderResult<Option<primitives::Nonce>> {
        self.0.nonce(address)
    }

    fn storage(
        &self,
        address: primitives::ContractAddress,
        storage_key: primitives::StorageKey,
    ) -> ProviderResult<Option<primitives::StorageValue>> {
        self.0.storage(address, storage_key)
    }
}

impl ContractClassProvider for StateRefDb {
    fn class(&self, hash: primitives::ClassHash) -> ProviderResult<Option<CompiledClass>> {
        self.0.class(hash)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: primitives::ClassHash,
    ) -> ProviderResult<Option<primitives::CompiledClassHash>> {
        self.0.compiled_class_hash_of_class_hash(hash)
    }

    fn sierra_class(
        &self,
        hash: primitives::ClassHash,
    ) -> ProviderResult<Option<FlattenedSierraClass>> {
        self.0.sierra_class(hash)
    }
}

impl StateReader for StateRefDb {
    fn get_nonce_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> StateResult<Nonce> {
        StateProvider::nonce(&self.0, contract_address.into())
            .map(|n| Nonce(n.unwrap_or_default().into()))
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_storage_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
        key: starknet_api::state::StorageKey,
    ) -> StateResult<starknet_api::hash::StarkFelt> {
        StateProvider::storage(&self.0, contract_address.into(), (*key.0.key()).into())
            .map(|v| v.unwrap_or_default().into())
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_class_hash_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> StateResult<starknet_api::core::ClassHash> {
        StateProvider::class_hash_of_contract(&self.0, contract_address.into())
            .map(|v| ClassHash(v.unwrap_or_default().into()))
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_compiled_class_hash(
        &mut self,
        class_hash: starknet_api::core::ClassHash,
    ) -> StateResult<starknet_api::core::CompiledClassHash> {
        if let Some(hash) =
            ContractClassProvider::compiled_class_hash_of_class_hash(&self.0, class_hash.0.into())
                .map_err(|e| StateError::StateReadError(e.to_string()))?
        {
            Ok(CompiledClassHash(hash.into()))
        } else {
            Err(StateError::UndeclaredClassHash(class_hash))
        }
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: starknet_api::core::ClassHash,
    ) -> StateResult<blockifier::execution::contract_class::ContractClass> {
        if let Some(class) = ContractClassProvider::class(&self.0, class_hash.0.into())
            .map_err(|e| StateError::StateReadError(e.to_string()))?
        {
            to_class(class).map_err(|e| StateError::StateReadError(e.to_string()))
        } else {
            Err(StateError::UndeclaredClassHash(class_hash))
        }
    }
}

#[derive(Default)]
pub struct ClassCache {
    pub(crate) compiled: HashMap<primitives::ClassHash, CompiledClass>,
    pub(crate) sierra: HashMap<primitives::ClassHash, FlattenedSierraClass>,
}

pub struct CachedStateWrapper<S: StateReader + StateProvider> {
    inner: Mutex<CachedState<S>>,
    pub(crate) class_cache: RwLock<ClassCache>,
}

impl<S> CachedStateWrapper<S>
where
    S: StateReader + StateProvider,
{
    pub fn new(db: S) -> Self {
        Self {
            class_cache: RwLock::new(ClassCache::default()),
            inner: Mutex::new(CachedState::new(db, GlobalContractCache::default())),
        }
    }

    pub(super) fn reset_with_new_state(&self, db: S) {
        *self.inner() = CachedState::new(db, GlobalContractCache::default());
        let mut lock = self.class_cache.write();
        lock.compiled.clear();
        lock.sierra.clear();
    }

    pub fn inner(&self) -> parking_lot::lock_api::MutexGuard<'_, RawMutex, CachedState<S>> {
        self.inner.lock()
    }
}

impl<Db> ContractClassProvider for CachedStateWrapper<Db>
where
    Db: StateReader + StateProvider + Sync + Send,
{
    fn class(
        &self,
        hash: katana_primitives::contract::ClassHash,
    ) -> ProviderResult<Option<CompiledClass>> {
        if let res @ Some(_) = self.class_cache.read().compiled.get(&hash).cloned() {
            Ok(res)
        } else {
            self.inner().state.class(hash)
        }
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: katana_primitives::contract::ClassHash,
    ) -> ProviderResult<Option<katana_primitives::contract::CompiledClassHash>> {
        let Ok(hash) = self.inner().get_compiled_class_hash(ClassHash(hash.into())) else {
            return Ok(None);
        };
        Ok(Some(hash.0.into()))
    }

    fn sierra_class(
        &self,
        hash: katana_primitives::contract::ClassHash,
    ) -> ProviderResult<Option<FlattenedSierraClass>> {
        let class = self.class_cache.read().sierra.get(&hash).cloned();
        Ok(class)
    }
}

impl<Db> StateProvider for CachedStateWrapper<Db>
where
    Db: StateReader + StateProvider + Sync + Send,
{
    fn storage(
        &self,
        address: katana_primitives::contract::ContractAddress,
        storage_key: katana_primitives::contract::StorageKey,
    ) -> ProviderResult<Option<katana_primitives::contract::StorageValue>> {
        let Ok(value) =
            self.inner().get_storage_at(address.into(), StorageKey(patricia_key!(storage_key)))
        else {
            return Ok(None);
        };
        Ok(Some(value.into()))
    }

    fn nonce(
        &self,
        address: katana_primitives::contract::ContractAddress,
    ) -> ProviderResult<Option<katana_primitives::contract::Nonce>> {
        let Ok(nonce) = self.inner().get_nonce_at(address.into()) else {
            return Ok(None);
        };
        Ok(Some(nonce.0.into()))
    }

    fn class_hash_of_contract(
        &self,
        address: katana_primitives::contract::ContractAddress,
    ) -> ProviderResult<Option<katana_primitives::contract::ClassHash>> {
        let Ok(hash) = self.inner().get_class_hash_at(address.into()) else {
            return Ok(None);
        };

        let hash = hash.0.into();
        if hash == FieldElement::ZERO { Ok(None) } else { Ok(Some(hash)) }
    }
}

use std::collections::HashMap;

use blockifier::state::cached_state::{CachedState, GlobalContractCache};
use blockifier::state::errors::StateError;
use blockifier::state::state_api::StateReader;
use katana_primitives::contract::SierraClass;
use katana_primitives::FieldElement;
use katana_provider::traits::contract::ContractClassProvider;
use katana_provider::traits::state::StateProvider;
use parking_lot::{Mutex, RawMutex, RwLock};
use starknet_api::core::{ClassHash, CompiledClassHash, Nonce, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;
use starknet_api::state::StorageKey;

/// A state db only provide read access.
///
/// This type implements the [`StateReader`] trait so that it can be used as a with [`CachedState`].
pub struct StateRefDb(Box<dyn StateProvider>);

impl StateRefDb {
    pub fn new(provider: impl StateProvider + 'static) -> Self {
        Self(Box::new(provider))
    }
}

impl<T> From<T> for StateRefDb
where
    T: StateProvider + 'static,
{
    fn from(provider: T) -> Self {
        Self::new(provider)
    }
}

impl StateReader for StateRefDb {
    fn get_nonce_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> blockifier::state::state_api::StateResult<Nonce> {
        StateProvider::nonce(&self.0, contract_address.into())
            .map(|n| Nonce(n.unwrap_or_default().into()))
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_storage_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
        key: starknet_api::state::StorageKey,
    ) -> blockifier::state::state_api::StateResult<starknet_api::hash::StarkFelt> {
        StateProvider::storage(&self.0, contract_address.into(), (*key.0.key()).into())
            .map(|v| v.unwrap_or_default().into())
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_class_hash_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> blockifier::state::state_api::StateResult<starknet_api::core::ClassHash> {
        StateProvider::class_hash_of_contract(&self.0, contract_address.into())
            .map(|v| ClassHash(v.unwrap_or_default().into()))
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_compiled_class_hash(
        &mut self,
        class_hash: starknet_api::core::ClassHash,
    ) -> blockifier::state::state_api::StateResult<starknet_api::core::CompiledClassHash> {
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
        class_hash: &starknet_api::core::ClassHash,
    ) -> blockifier::state::state_api::StateResult<
        blockifier::execution::contract_class::ContractClass,
    > {
        if let Some(class) = ContractClassProvider::class(&self.0, class_hash.0.into())
            .map_err(|e| StateError::StateReadError(e.to_string()))?
        {
            Ok(class)
        } else {
            Err(StateError::UndeclaredClassHash(*class_hash))
        }
    }
}

pub struct CachedStateWrapper<S: StateReader> {
    inner: Mutex<CachedState<S>>,
    sierra_class: RwLock<HashMap<katana_primitives::contract::ClassHash, SierraClass>>,
}

impl<S: StateReader> CachedStateWrapper<S> {
    pub fn new(db: S) -> Self {
        Self {
            sierra_class: Default::default(),
            inner: Mutex::new(CachedState::new(db, GlobalContractCache::default())),
        }
    }

    pub(super) fn reset_with_new_state(&self, db: S) {
        *self.inner() = CachedState::new(db, GlobalContractCache::default());
        self.sierra_class_mut().clear();
    }

    pub fn inner(&self) -> parking_lot::lock_api::MutexGuard<'_, RawMutex, CachedState<S>> {
        self.inner.lock()
    }

    pub fn sierra_class(
        &self,
    ) -> parking_lot::RwLockReadGuard<
        '_,
        HashMap<katana_primitives::contract::ClassHash, SierraClass>,
    > {
        self.sierra_class.read()
    }

    pub fn sierra_class_mut(
        &self,
    ) -> parking_lot::RwLockWriteGuard<
        '_,
        HashMap<katana_primitives::contract::ClassHash, SierraClass>,
    > {
        self.sierra_class.write()
    }
}

impl<Db> ContractClassProvider for CachedStateWrapper<Db>
where
    Db: StateReader + Sync + Send,
{
    fn class(
        &self,
        hash: katana_primitives::contract::ClassHash,
    ) -> anyhow::Result<Option<katana_primitives::contract::CompiledContractClass>> {
        let Ok(class) = self.inner().get_compiled_contract_class(&ClassHash(hash.into())) else {
            return Ok(None);
        };
        Ok(Some(class))
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: katana_primitives::contract::ClassHash,
    ) -> anyhow::Result<Option<katana_primitives::contract::CompiledClassHash>> {
        let Ok(hash) = self.inner().get_compiled_class_hash(ClassHash(hash.into())) else {
            return Ok(None);
        };
        Ok(Some(hash.0.into()))
    }

    fn sierra_class(
        &self,
        hash: katana_primitives::contract::ClassHash,
    ) -> anyhow::Result<Option<SierraClass>> {
        let class @ Some(_) = self.sierra_class().get(&hash).cloned() else {
            return Ok(None);
        };
        Ok(class)
    }
}

impl<Db> StateProvider for CachedStateWrapper<Db>
where
    Db: StateReader + Sync + Send,
{
    fn storage(
        &self,
        address: katana_primitives::contract::ContractAddress,
        storage_key: katana_primitives::contract::StorageKey,
    ) -> anyhow::Result<Option<katana_primitives::contract::StorageValue>> {
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
    ) -> anyhow::Result<Option<katana_primitives::contract::Nonce>> {
        let Ok(nonce) = self.inner().get_nonce_at(address.into()) else {
            return Ok(None);
        };
        Ok(Some(nonce.0.into()))
    }

    fn class_hash_of_contract(
        &self,
        address: katana_primitives::contract::ContractAddress,
    ) -> anyhow::Result<Option<katana_primitives::contract::ClassHash>> {
        let Ok(hash) = self.inner().get_class_hash_at(address.into()) else {
            return Ok(None);
        };

        let hash = hash.0.into();
        if hash == FieldElement::ZERO { Ok(None) } else { Ok(Some(hash)) }
    }
}

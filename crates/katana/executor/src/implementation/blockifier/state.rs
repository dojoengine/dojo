use std::collections::HashMap;
use std::sync::Arc;

use blockifier::state::cached_state::{self, GlobalContractCache};
use blockifier::state::errors::StateError;
use blockifier::state::state_api::{StateReader, StateResult};
use katana_primitives::class::{CompiledClass, FlattenedSierraClass};
use katana_primitives::{conversion, FieldElement};
use katana_provider::traits::contract::ContractClassProvider;
use katana_provider::traits::state::StateProvider;
use katana_provider::ProviderResult;
use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use starknet_api::core::{ClassHash, CompiledClassHash, Nonce, PatriciaKey};
use starknet_api::hash::StarkHash;
use starknet_api::patricia_key;
use starknet_api::state::StorageKey;

use crate::StateProviderDb;

/// A helper trait to enforce that a type must implement both [StateProvider] and [StateReader].
pub(super) trait StateDb: StateProvider + StateReader {}

impl<T> StateDb for T where T: StateProvider + StateReader {}

impl<'a> StateReader for StateProviderDb<'a> {
    fn get_class_hash_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> StateResult<starknet_api::core::ClassHash> {
        self.0
            .class_hash_of_contract(contract_address.into())
            .map(|v| ClassHash(v.unwrap_or_default().into()))
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_compiled_class_hash(
        &mut self,
        class_hash: starknet_api::core::ClassHash,
    ) -> StateResult<starknet_api::core::CompiledClassHash> {
        if let Some(hash) = self
            .0
            .compiled_class_hash_of_class_hash(class_hash.0.into())
            .map_err(|e| StateError::StateReadError(e.to_string()))?
        {
            Ok(CompiledClassHash(hash.into()))
        } else {
            Err(StateError::UndeclaredClassHash(class_hash))
        }
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: ClassHash,
    ) -> StateResult<blockifier::execution::contract_class::ContractClass> {
        if let Some(class) = self
            .0
            .class(class_hash.0.into())
            .map_err(|e| StateError::StateReadError(e.to_string()))?
        {
            let class = conversion::blockifier::to_class(class)
                .map_err(|e| StateError::StateReadError(e.to_string()))?;

            Ok(class)
        } else {
            Err(StateError::UndeclaredClassHash(class_hash))
        }
    }

    fn get_nonce_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> StateResult<starknet_api::core::Nonce> {
        self.0
            .nonce(contract_address.into())
            .map(|n| Nonce(n.unwrap_or_default().into()))
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }

    fn get_storage_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
        key: starknet_api::state::StorageKey,
    ) -> StateResult<starknet_api::hash::StarkFelt> {
        self.0
            .storage(contract_address.into(), (*key.0.key()).into())
            .map(|v| v.unwrap_or_default().into())
            .map_err(|e| StateError::StateReadError(e.to_string()))
    }
}

pub(super) struct CachedState<S: StateDb>(pub(super) Arc<RwLock<CachedStateInner<S>>>);

impl<S: StateDb> Clone for CachedState<S> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

type DeclaredClass = (CompiledClass, Option<FlattenedSierraClass>);

#[derive(Debug)]
pub(super) struct CachedStateInner<S: StateReader> {
    pub(super) inner: cached_state::CachedState<S>,
    pub(super) declared_classes: HashMap<katana_primitives::class::ClassHash, DeclaredClass>,
}

impl<S: StateDb> CachedState<S> {
    pub(super) fn new(state: S) -> Self {
        let declared_classes = HashMap::new();
        let cached_state = cached_state::CachedState::new(state, GlobalContractCache::default());
        let inner = CachedStateInner { inner: cached_state, declared_classes };
        Self(Arc::new(RwLock::new(inner)))
    }

    fn read(&self) -> RwLockReadGuard<'_, CachedStateInner<S>> {
        self.0.read()
    }

    fn write(&self) -> RwLockWriteGuard<'_, CachedStateInner<S>> {
        self.0.write()
    }
}

impl<S: StateDb> ContractClassProvider for CachedState<S> {
    fn class(
        &self,
        hash: katana_primitives::class::ClassHash,
    ) -> ProviderResult<Option<CompiledClass>> {
        let state = self.read();
        if let Some((class, _)) = state.declared_classes.get(&hash) {
            Ok(Some(class.clone()))
        } else {
            state.inner.state.class(hash)
        }
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: katana_primitives::class::ClassHash,
    ) -> ProviderResult<Option<katana_primitives::class::CompiledClassHash>> {
        let Ok(hash) = self.write().inner.get_compiled_class_hash(ClassHash(hash.into())) else {
            return Ok(None);
        };
        Ok(Some(hash.0.into()))
    }

    fn sierra_class(
        &self,
        hash: katana_primitives::class::ClassHash,
    ) -> ProviderResult<Option<FlattenedSierraClass>> {
        let state = self.read();
        if let Some((_, sierra)) = state.declared_classes.get(&hash) {
            Ok(sierra.clone())
        } else {
            state.inner.state.sierra_class(hash)
        }
    }
}

impl<S: StateDb> StateProvider for CachedState<S> {
    fn class_hash_of_contract(
        &self,
        address: katana_primitives::contract::ContractAddress,
    ) -> ProviderResult<Option<katana_primitives::class::ClassHash>> {
        let Ok(hash) = self.write().inner.get_class_hash_at(address.into()) else {
            return Ok(None);
        };

        let hash = hash.0.into();
        if hash == FieldElement::ZERO { Ok(None) } else { Ok(Some(hash)) }
    }

    fn nonce(
        &self,
        address: katana_primitives::contract::ContractAddress,
    ) -> ProviderResult<Option<katana_primitives::contract::Nonce>> {
        let Ok(nonce) = self.write().inner.get_nonce_at(address.into()) else {
            return Ok(None);
        };
        Ok(Some(nonce.0.into()))
    }

    fn storage(
        &self,
        address: katana_primitives::contract::ContractAddress,
        storage_key: katana_primitives::contract::StorageKey,
    ) -> ProviderResult<Option<katana_primitives::contract::StorageValue>> {
        let address = address.into();
        let key = StorageKey(patricia_key!(storage_key));

        if let Ok(value) = self.write().inner.get_storage_at(address, key) {
            Ok(Some(value.into()))
        } else {
            Ok(None)
        }
    }
}

impl<S: StateDb> StateReader for CachedState<S> {
    fn get_class_hash_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> StateResult<ClassHash> {
        self.write().inner.get_class_hash_at(contract_address)
    }

    fn get_compiled_class_hash(&mut self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        self.write().inner.get_compiled_class_hash(class_hash)
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: ClassHash,
    ) -> StateResult<blockifier::execution::contract_class::ContractClass> {
        self.write().inner.get_compiled_contract_class(class_hash)
    }

    fn get_nonce_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
    ) -> StateResult<Nonce> {
        self.write().inner.get_nonce_at(contract_address)
    }

    fn get_storage_at(
        &mut self,
        contract_address: starknet_api::core::ContractAddress,
        key: StorageKey,
    ) -> StateResult<starknet_api::hash::StarkFelt> {
        self.write().inner.get_storage_at(contract_address, key)
    }
}

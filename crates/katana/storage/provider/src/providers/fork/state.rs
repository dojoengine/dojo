use std::sync::Arc;

use katana_primitives::class::{ClassHash, CompiledClass, CompiledClassHash, FlattenedSierraClass};
use katana_primitives::contract::{ContractAddress, Nonce, StorageKey, StorageValue};

use super::backend::SharedStateProvider;
use crate::providers::in_memory::cache::CacheStateDb;
use crate::providers::in_memory::state::StateSnapshot;
use crate::traits::contract::ContractClassProvider;
use crate::traits::state::StateProvider;
use crate::ProviderResult;

pub type ForkedStateDb = CacheStateDb<SharedStateProvider>;
pub type ForkedSnapshot = StateSnapshot<SharedStateProvider>;

impl ForkedStateDb {
    pub(crate) fn create_snapshot(&self) -> ForkedSnapshot {
        ForkedSnapshot {
            inner: self.create_snapshot_without_classes(),
            classes: Arc::clone(&self.shared_contract_classes),
        }
    }
}

impl StateProvider for ForkedStateDb {
    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<ClassHash>> {
        if let hash @ Some(_) = self
            .contract_state
            .read()
            .get(&address)
            .map(|i| i.class_hash)
            .filter(|h| h != &ClassHash::ZERO)
        {
            return Ok(hash);
        }
        StateProvider::class_hash_of_contract(&self.db, address)
    }

    // TODO: write unit tests for these cases
    //
    // When reading from local storage, we only consider entries that have non-zero nonce
    // values OR non-zero class hashes.
    //
    // Nonce == 0 && ClassHash == 0
    // - Contract does not exist locally (so try find from remote state)
    // Nonce != 0 && ClassHash == 0
    // - Contract exists and was deployed remotely but new nonce was set locally (so no need to read
    //   from remote state anymore)
    // Nonce == 0 && ClassHash != 0
    // - Contract exists and was deployed locally (always read from local state)
    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        if let nonce @ Some(_) = self
            .contract_state
            .read()
            .get(&address)
            .filter(|c| c.nonce != Nonce::default() || c.class_hash != ClassHash::default())
            .map(|c| c.nonce)
        {
            return Ok(nonce);
        }
        StateProvider::nonce(&self.db, address)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        if let value @ Some(_) =
            self.storage.read().get(&address).and_then(|s| s.get(&storage_key)).copied()
        {
            return Ok(value);
        }
        StateProvider::storage(&self.db, address, storage_key)
    }
}

impl ContractClassProvider for CacheStateDb<SharedStateProvider> {
    fn sierra_class(&self, hash: ClassHash) -> ProviderResult<Option<FlattenedSierraClass>> {
        if let class @ Some(_) = self.shared_contract_classes.sierra_classes.read().get(&hash) {
            return Ok(class.cloned());
        }
        ContractClassProvider::sierra_class(&self.db, hash)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        if let hash @ Some(_) = self.compiled_class_hashes.read().get(&hash) {
            return Ok(hash.cloned());
        }
        ContractClassProvider::compiled_class_hash_of_class_hash(&self.db, hash)
    }

    fn class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        if let class @ Some(_) = self.shared_contract_classes.compiled_classes.read().get(&hash) {
            return Ok(class.cloned());
        }
        ContractClassProvider::class(&self.db, hash)
    }
}

pub(super) struct LatestStateProvider(pub(super) Arc<ForkedStateDb>);

impl StateProvider for LatestStateProvider {
    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        StateProvider::nonce(&self.0, address)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        StateProvider::storage(&self.0, address, storage_key)
    }

    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<ClassHash>> {
        StateProvider::class_hash_of_contract(&self.0, address)
    }
}

impl ContractClassProvider for LatestStateProvider {
    fn sierra_class(&self, hash: ClassHash) -> ProviderResult<Option<FlattenedSierraClass>> {
        ContractClassProvider::sierra_class(&self.0, hash)
    }

    fn class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        ContractClassProvider::class(&self.0, hash)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        ContractClassProvider::compiled_class_hash_of_class_hash(&self.0, hash)
    }
}

impl StateProvider for ForkedSnapshot {
    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>> {
        if let nonce @ Some(_) = self
            .inner
            .contract_state
            .get(&address)
            .map(|info| info.nonce)
            .filter(|n| n != &Nonce::ZERO)
        {
            return Ok(nonce);
        }
        StateProvider::nonce(&self.inner.db, address)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>> {
        if let value @ Some(_) =
            self.inner.storage.get(&address).and_then(|s| s.get(&storage_key)).copied()
        {
            return Ok(value);
        }
        StateProvider::storage(&self.inner.db, address, storage_key)
    }

    fn class_hash_of_contract(
        &self,
        address: ContractAddress,
    ) -> ProviderResult<Option<ClassHash>> {
        if let class_hash @ Some(_) = self
            .inner
            .contract_state
            .get(&address)
            .map(|info| info.class_hash)
            .filter(|h| h != &ClassHash::ZERO)
        {
            return Ok(class_hash);
        }
        StateProvider::class_hash_of_contract(&self.inner.db, address)
    }
}

impl ContractClassProvider for ForkedSnapshot {
    fn sierra_class(&self, hash: ClassHash) -> ProviderResult<Option<FlattenedSierraClass>> {
        if self.inner.compiled_class_hashes.get(&hash).is_some() {
            Ok(self.classes.sierra_classes.read().get(&hash).cloned())
        } else {
            ContractClassProvider::sierra_class(&self.inner.db, hash)
        }
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> ProviderResult<Option<CompiledClassHash>> {
        if let hash @ Some(_) = self.inner.compiled_class_hashes.get(&hash).cloned() {
            return Ok(hash);
        }
        ContractClassProvider::compiled_class_hash_of_class_hash(&self.inner.db, hash)
    }

    fn class(&self, hash: ClassHash) -> ProviderResult<Option<CompiledClass>> {
        if self.inner.compiled_class_hashes.get(&hash).is_some() {
            Ok(self.classes.compiled_classes.read().get(&hash).cloned())
        } else {
            ContractClassProvider::class(&self.inner.db, hash)
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use std::collections::HashMap;

//     use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};

//     use crate::providers::{
//         fork::backend::{Backend, SharedStateProvider},
//         in_memory::cache::CacheStateDb,
//     };

//     use super::ForkedStateDb;

//     #[test]
//     fn test_get_nonce() {
//         let backend = Backend::new().unwrap();

//         let mut state = CacheStateDb::new(backend);
//         state.insert_updates();

//         let forked_state = SharedStateProvider::new_with_state(state);

//         // setup initial state
//         let states = StateUpdatesWithDeclaredClasses {
//             state_updates: StateUpdates { nonce_updates: HashMap::from([()]) },
//             ..Default::default()
//         };
//     }
}

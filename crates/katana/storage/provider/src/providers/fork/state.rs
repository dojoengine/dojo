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

#[derive(Debug)]
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
            .filter(|c| c.nonce != Nonce::default() || c.class_hash != ClassHash::default())
            .map(|c| c.nonce)
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
        if self.inner.compiled_class_hashes.contains_key(&hash) {
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
        if self.inner.compiled_class_hashes.contains_key(&hash) {
            Ok(self.classes.compiled_classes.read().get(&hash).cloned())
        } else {
            ContractClassProvider::class(&self.inner.db, hash)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use katana_primitives::state::{StateUpdates, StateUpdatesWithDeclaredClasses};
    use starknet::macros::felt;

    use super::*;
    use crate::providers::fork::backend::test_utils::create_forked_backend;

    #[test]
    fn test_get_nonce() {
        let backend = create_forked_backend("http://localhost:8080", 1);

        let address: ContractAddress = felt!("1").into();
        let class_hash = felt!("11");
        let remote_nonce = felt!("111");
        let local_nonce = felt!("1111");

        // Case: contract doesn't exist at all
        {
            let remote = SharedStateProvider::new_with_backend(backend.clone());
            let local = ForkedStateDb::new(remote.clone());

            // asserts that its error for now
            assert!(local.nonce(address).is_err());
            assert!(remote.nonce(address).is_err());

            // make sure the snapshot maintains the same behavior
            let snapshot = local.create_snapshot();
            assert!(snapshot.nonce(address).is_err());
        }

        // Case: contract exist remotely
        {
            let remote = SharedStateProvider::new_with_backend(backend.clone());
            let local = ForkedStateDb::new(remote.clone());

            let nonce_updates = BTreeMap::from([(address, remote_nonce)]);
            let updates = StateUpdatesWithDeclaredClasses {
                state_updates: StateUpdates { nonce_updates, ..Default::default() },
                ..Default::default()
            };
            remote.0.insert_updates(updates);

            assert_eq!(local.nonce(address).unwrap(), Some(remote_nonce));
            assert_eq!(remote.nonce(address).unwrap(), Some(remote_nonce));

            // make sure the snapshot maintains the same behavior
            let snapshot = local.create_snapshot();
            assert_eq!(snapshot.nonce(address).unwrap(), Some(remote_nonce));
        }

        // Case: contract exist remotely but nonce was updated locally
        {
            let remote = SharedStateProvider::new_with_backend(backend.clone());
            let local = ForkedStateDb::new(remote.clone());

            let nonce_updates = BTreeMap::from([(address, remote_nonce)]);
            let deployed_contracts = BTreeMap::from([(address, class_hash)]);
            let updates = StateUpdatesWithDeclaredClasses {
                state_updates: StateUpdates {
                    nonce_updates,
                    deployed_contracts,
                    ..Default::default()
                },
                ..Default::default()
            };
            remote.0.insert_updates(updates);

            let nonce_updates = BTreeMap::from([(address, local_nonce)]);
            let updates = StateUpdatesWithDeclaredClasses {
                state_updates: StateUpdates { nonce_updates, ..Default::default() },
                ..Default::default()
            };
            local.insert_updates(updates);

            assert_eq!(local.nonce(address).unwrap(), Some(local_nonce));
            assert_eq!(remote.nonce(address).unwrap(), Some(remote_nonce));

            // make sure the snapshot maintains the same behavior
            let snapshot = local.create_snapshot();
            assert_eq!(snapshot.nonce(address).unwrap(), Some(local_nonce));
        }

        // Case: contract was deployed locally only and has non-zero nonce
        {
            let remote = SharedStateProvider::new_with_backend(backend.clone());
            let local = ForkedStateDb::new(remote.clone());

            let deployed_contracts = BTreeMap::from([(address, class_hash)]);
            let nonce_updates = BTreeMap::from([(address, local_nonce)]);
            let updates = StateUpdatesWithDeclaredClasses {
                state_updates: StateUpdates {
                    nonce_updates,
                    deployed_contracts,
                    ..Default::default()
                },
                ..Default::default()
            };
            local.insert_updates(updates);

            assert_eq!(local.nonce(address).unwrap(), Some(local_nonce));
            assert!(remote.nonce(address).is_err());

            // make sure the snapshot maintains the same behavior
            let snapshot = local.create_snapshot();
            assert_eq!(snapshot.nonce(address).unwrap(), Some(local_nonce));
        }

        // Case: contract was deployed locally only and has zero nonce
        {
            let remote = SharedStateProvider::new_with_backend(backend.clone());
            let local = ForkedStateDb::new(remote.clone());

            let deployed_contracts = BTreeMap::from([(address, class_hash)]);
            let updates = StateUpdatesWithDeclaredClasses {
                state_updates: StateUpdates { deployed_contracts, ..Default::default() },
                ..Default::default()
            };
            local.insert_updates(updates);

            assert_eq!(local.nonce(address).unwrap(), Some(Default::default()));
            assert!(remote.nonce(address).is_err());

            // make sure the snapshot maintains the same behavior
            let snapshot = local.create_snapshot();
            assert_eq!(snapshot.nonce(address).unwrap(), Some(Default::default()));
        }
    }
}

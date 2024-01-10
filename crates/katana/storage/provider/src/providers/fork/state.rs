use std::sync::Arc;

use anyhow::Result;
use katana_primitives::contract::{
    ClassHash, CompiledClassHash, CompiledContractClass, ContractAddress, FlattenedSierraClass,
    GenericContractInfo, Nonce, StorageKey, StorageValue,
};

use super::backend::SharedStateProvider;
use crate::providers::in_memory::cache::CacheStateDb;
use crate::providers::in_memory::state::StateSnapshot;
use crate::traits::contract::{ContractClassProvider, ContractInfoProvider};
use crate::traits::state::StateProvider;

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

impl ContractInfoProvider for ForkedStateDb {
    fn contract(&self, address: ContractAddress) -> Result<Option<GenericContractInfo>> {
        if let info @ Some(_) = self.contract_state.read().get(&address).cloned() {
            return Ok(info);
        }
        ContractInfoProvider::contract(&self.db, address)
    }
}

impl StateProvider for ForkedStateDb {
    fn class_hash_of_contract(&self, address: ContractAddress) -> Result<Option<ClassHash>> {
        if let hash @ Some(_) = self.contract_state.read().get(&address).map(|i| i.class_hash) {
            return Ok(hash);
        }
        StateProvider::class_hash_of_contract(&self.db, address)
    }

    fn nonce(&self, address: ContractAddress) -> Result<Option<Nonce>> {
        if let nonce @ Some(_) = self.contract_state.read().get(&address).map(|i| i.nonce) {
            return Ok(nonce);
        }
        StateProvider::nonce(&self.db, address)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> Result<Option<StorageValue>> {
        if let value @ Some(_) =
            self.storage.read().get(&address).and_then(|s| s.get(&storage_key)).copied()
        {
            return Ok(value);
        }
        StateProvider::storage(&self.db, address, storage_key)
    }
}

impl ContractClassProvider for CacheStateDb<SharedStateProvider> {
    fn sierra_class(&self, hash: ClassHash) -> Result<Option<FlattenedSierraClass>> {
        if let class @ Some(_) = self.shared_contract_classes.sierra_classes.read().get(&hash) {
            return Ok(class.cloned());
        }
        ContractClassProvider::sierra_class(&self.db, hash)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> Result<Option<CompiledClassHash>> {
        if let hash @ Some(_) = self.compiled_class_hashes.read().get(&hash) {
            return Ok(hash.cloned());
        }
        ContractClassProvider::compiled_class_hash_of_class_hash(&self.db, hash)
    }

    fn class(&self, hash: ClassHash) -> Result<Option<CompiledContractClass>> {
        if let class @ Some(_) = self.shared_contract_classes.compiled_classes.read().get(&hash) {
            return Ok(class.cloned());
        }
        ContractClassProvider::class(&self.db, hash)
    }
}

pub(super) struct LatestStateProvider(pub(super) Arc<ForkedStateDb>);

impl ContractInfoProvider for LatestStateProvider {
    fn contract(&self, address: ContractAddress) -> Result<Option<GenericContractInfo>> {
        ContractInfoProvider::contract(&self.0, address)
    }
}

impl StateProvider for LatestStateProvider {
    fn nonce(&self, address: ContractAddress) -> Result<Option<Nonce>> {
        StateProvider::nonce(&self.0, address)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> Result<Option<StorageValue>> {
        StateProvider::storage(&self.0, address, storage_key)
    }

    fn class_hash_of_contract(&self, address: ContractAddress) -> Result<Option<ClassHash>> {
        StateProvider::class_hash_of_contract(&self.0, address)
    }
}

impl ContractClassProvider for LatestStateProvider {
    fn sierra_class(&self, hash: ClassHash) -> Result<Option<FlattenedSierraClass>> {
        ContractClassProvider::sierra_class(&self.0, hash)
    }

    fn class(&self, hash: ClassHash) -> Result<Option<CompiledContractClass>> {
        ContractClassProvider::class(&self.0, hash)
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> Result<Option<CompiledClassHash>> {
        ContractClassProvider::compiled_class_hash_of_class_hash(&self.0, hash)
    }
}

impl ContractInfoProvider for ForkedSnapshot {
    fn contract(&self, address: ContractAddress) -> Result<Option<GenericContractInfo>> {
        if let info @ Some(_) = self.inner.contract_state.get(&address).cloned() {
            return Ok(info);
        }
        ContractInfoProvider::contract(&self.inner.db, address)
    }
}

impl StateProvider for ForkedSnapshot {
    fn nonce(&self, address: ContractAddress) -> Result<Option<Nonce>> {
        if let nonce @ Some(_) = self.inner.contract_state.get(&address).map(|info| info.nonce) {
            return Ok(nonce);
        }
        StateProvider::nonce(&self.inner.db, address)
    }

    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> Result<Option<StorageValue>> {
        if let value @ Some(_) =
            self.inner.storage.get(&address).and_then(|s| s.get(&storage_key)).copied()
        {
            return Ok(value);
        }
        StateProvider::storage(&self.inner.db, address, storage_key)
    }

    fn class_hash_of_contract(&self, address: ContractAddress) -> Result<Option<ClassHash>> {
        if let class_hash @ Some(_) =
            self.inner.contract_state.get(&address).map(|info| info.class_hash)
        {
            return Ok(class_hash);
        }
        StateProvider::class_hash_of_contract(&self.inner.db, address)
    }
}

impl ContractClassProvider for ForkedSnapshot {
    fn sierra_class(&self, hash: ClassHash) -> Result<Option<FlattenedSierraClass>> {
        if self.inner.compiled_class_hashes.get(&hash).is_some() {
            Ok(self.classes.sierra_classes.read().get(&hash).cloned())
        } else {
            ContractClassProvider::sierra_class(&self.inner.db, hash)
        }
    }

    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> Result<Option<CompiledClassHash>> {
        if let hash @ Some(_) = self.inner.compiled_class_hashes.get(&hash).cloned() {
            return Ok(hash);
        }
        ContractClassProvider::compiled_class_hash_of_class_hash(&self.inner.db, hash)
    }

    fn class(&self, hash: ClassHash) -> Result<Option<CompiledContractClass>> {
        if self.inner.compiled_class_hashes.get(&hash).is_some() {
            Ok(self.classes.compiled_classes.read().get(&hash).cloned())
        } else {
            ContractClassProvider::class(&self.inner.db, hash)
        }
    }
}

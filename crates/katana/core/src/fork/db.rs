use std::collections::BTreeMap;
use std::sync::Arc;

use blockifier::execution::contract_class::ContractClass;
use blockifier::state::cached_state::CommitmentStateDiff;
use blockifier::state::state_api::{State, StateReader, StateResult};
use starknet::core::types::{BlockId, FlattenedSierraClass};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet_api::core::{ClassHash, CompiledClassHash, ContractAddress, Nonce};
use starknet_api::hash::StarkFelt;
use starknet_api::state::StorageKey;

use super::backend::SharedBackend;
use crate::db::cached::CachedDb;
use crate::db::serde::state::{
    SerializableClassRecord, SerializableState, SerializableStorageRecord,
};
use crate::db::{AsStateRefDb, Database, StateExt, StateExtRef, StateRefDb};

/// A state database implementation that forks from a network.
///
/// It will try to find the requested data in the cache, and if it's not there, it will fetch it
/// from the forked network. The fetched data will be stored in the cache so that the next time the
/// same data is requested, it will be fetched from the cache instead of fetching it from the forked
/// network again.
///
/// The forked database provider should be locked to a particular block.
#[derive(Debug, Clone)]
pub struct ForkedDb {
    /// Shared cache of the forked database. This will be shared across all instances of the
    /// `ForkedDb` when it is cloned into a [StateRefDb] using the [AsStateRefDb] trait.
    ///
    /// So if one instance fetches data from the forked network, the
    /// other instances will be able to use the cached data instead of fetching it again.
    db: CachedDb<SharedBackend>,
}

impl ForkedDb {
    /// Construct a new `ForkedDb` from a `Provider` of the network to fork from at a particular
    /// `block`.
    pub fn new(provider: Arc<JsonRpcClient<HttpTransport>>, block: BlockId) -> Self {
        Self { db: CachedDb::new(SharedBackend::new_with_backend_thread(provider, block)) }
    }

    #[cfg(test)]
    pub fn new_from_backend(db: CachedDb<SharedBackend>) -> Self {
        Self { db }
    }
}

impl State for ForkedDb {
    fn set_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
        value: StarkFelt,
    ) {
        self.db.set_storage_at(contract_address, key, value);
    }

    fn set_class_hash_at(
        &mut self,
        contract_address: ContractAddress,
        class_hash: ClassHash,
    ) -> StateResult<()> {
        self.db.set_class_hash_at(contract_address, class_hash)
    }

    fn set_compiled_class_hash(
        &mut self,
        class_hash: ClassHash,
        compiled_class_hash: CompiledClassHash,
    ) -> StateResult<()> {
        self.db.set_compiled_class_hash(class_hash, compiled_class_hash)
    }

    fn to_state_diff(&self) -> CommitmentStateDiff {
        self.db.to_state_diff()
    }

    fn set_contract_class(
        &mut self,
        class_hash: &ClassHash,
        contract_class: ContractClass,
    ) -> StateResult<()> {
        self.db.set_contract_class(class_hash, contract_class)
    }

    fn increment_nonce(&mut self, contract_address: ContractAddress) -> StateResult<()> {
        self.db.increment_nonce(contract_address)
    }
}

impl StateReader for ForkedDb {
    fn get_nonce_at(&mut self, contract_address: ContractAddress) -> StateResult<Nonce> {
        let nonce = self.db.get_nonce_at(contract_address)?;
        Ok(nonce)
    }

    fn get_storage_at(
        &mut self,
        contract_address: ContractAddress,
        key: StorageKey,
    ) -> StateResult<StarkFelt> {
        self.db.get_storage_at(contract_address, key)
    }

    fn get_class_hash_at(
        &mut self,
        contract_address: ContractAddress,
    ) -> StateResult<starknet_api::core::ClassHash> {
        self.db.get_class_hash_at(contract_address)
    }

    fn get_compiled_class_hash(&mut self, class_hash: ClassHash) -> StateResult<CompiledClassHash> {
        self.db.get_compiled_class_hash(class_hash)
    }

    fn get_compiled_contract_class(
        &mut self,
        class_hash: &ClassHash,
    ) -> StateResult<ContractClass> {
        self.db.get_compiled_contract_class(class_hash)
    }
}

impl StateExtRef for ForkedDb {
    fn get_sierra_class(&mut self, class_hash: &ClassHash) -> StateResult<FlattenedSierraClass> {
        self.db.get_sierra_class(class_hash)
    }
}

impl StateExt for ForkedDb {
    fn set_sierra_class(
        &mut self,
        class_hash: ClassHash,
        sierra_class: FlattenedSierraClass,
    ) -> StateResult<()> {
        self.db.set_sierra_class(class_hash, sierra_class)
    }
}

impl AsStateRefDb for ForkedDb {
    fn as_ref_db(&self) -> StateRefDb {
        StateRefDb::new(self.clone())
    }
}

impl Database for ForkedDb {
    fn set_nonce(&mut self, addr: ContractAddress, nonce: Nonce) {
        self.db.storage.entry(addr).or_default().nonce = nonce;
    }

    fn dump_state(&self) -> anyhow::Result<SerializableState> {
        let mut serializable = SerializableState::default();

        self.db.storage.iter().for_each(|(addr, storage)| {
            let mut record = SerializableStorageRecord {
                storage: BTreeMap::new(),
                nonce: storage.nonce.0.into(),
            };

            storage.storage.iter().for_each(|(key, value)| {
                record.storage.insert((*key.0.key()).into(), (*value).into());
            });

            serializable.storage.insert((*addr.0.key()).into(), record);
        });

        self.db.classes.iter().for_each(|(class_hash, class_record)| {
            serializable.classes.insert(
                class_hash.0.into(),
                SerializableClassRecord {
                    class: class_record.class.clone().into(),
                    compiled_hash: class_record.compiled_hash.0.into(),
                },
            );
        });

        self.db.contracts.iter().for_each(|(address, class_hash)| {
            serializable.contracts.insert((*address.0.key()).into(), class_hash.0.into());
        });

        self.db.sierra_classes.iter().for_each(|(class_hash, class)| {
            serializable.sierra_classes.insert(class_hash.0.into(), class.clone());
        });

        Ok(serializable)
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::types::BlockTag;
    use starknet::providers::jsonrpc::HttpTransport;
    use starknet::providers::JsonRpcClient;
    use starknet_api::core::PatriciaKey;
    use starknet_api::hash::StarkHash;
    use starknet_api::{patricia_key, stark_felt};
    use url::Url;

    use super::*;
    use crate::constants::UDC_CONTRACT;

    const FORKED_ENDPOINT: &str =
        "https://starknet-goerli.infura.io/v3/369ce5ac40614952af936e4d64e40474";

    #[tokio::test]
    async fn fetch_from_cache_if_exist() {
        let address = ContractAddress(patricia_key!(0x1u32));
        let class_hash = ClassHash(stark_felt!(0x88u32));

        let expected_nonce = Nonce(stark_felt!(44u32));
        let expected_storage_key = StorageKey(patricia_key!(0x2u32));
        let expected_storage_value = stark_felt!(55u32);
        let expected_compiled_class_hash = CompiledClassHash(class_hash.0);
        let expected_contract_class = (*UDC_CONTRACT).clone();

        let provider = JsonRpcClient::new(HttpTransport::new(Url::parse(FORKED_ENDPOINT).unwrap()));
        let mut cache = CachedDb::new(SharedBackend::new_with_backend_thread(
            Arc::new(provider),
            BlockId::Tag(BlockTag::Latest),
        ));

        cache.storage.entry(address).or_default().nonce = expected_nonce;
        cache.set_storage_at(address, expected_storage_key, expected_storage_value);
        cache.set_contract_class(&class_hash, expected_contract_class.clone()).unwrap();
        cache.set_compiled_class_hash(class_hash, expected_compiled_class_hash).unwrap();

        let mut db = ForkedDb::new_from_backend(cache);

        let nonce = db.get_nonce_at(address).unwrap();
        let storage_value = db.get_storage_at(address, expected_storage_key).unwrap();
        let contract_class = db.get_compiled_contract_class(&class_hash).unwrap();
        let compiled_class_hash = db.get_compiled_class_hash(class_hash).unwrap();

        assert_eq!(nonce, expected_nonce);
        assert_eq!(storage_value, expected_storage_value);
        assert_eq!(contract_class, expected_contract_class);
        assert_eq!(compiled_class_hash, expected_compiled_class_hash)
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn fetch_from_provider_if_not_in_cache() {
        let provider = JsonRpcClient::new(HttpTransport::new(Url::parse(FORKED_ENDPOINT).unwrap()));
        let mut db = ForkedDb::new(Arc::new(provider), BlockId::Tag(BlockTag::Latest));

        let address = ContractAddress(patricia_key!(
            "0x02b92ec12cA1e308f320e99364d4dd8fcc9efDAc574F836C8908de937C289974"
        ));
        let storage_key = StorageKey(patricia_key!(
            "0x3b459c3fadecdb1a501f2fdeec06fd735cb2d93ea59779177a0981660a85352"
        ));

        let class_hash = db.get_class_hash_at(address).unwrap();
        let class = db.get_compiled_contract_class(&class_hash).unwrap();
        let storage_value = db.get_storage_at(address, storage_key).unwrap();

        let expected_class_hash = ClassHash(stark_felt!(
            "0x01a736d6ed154502257f02b1ccdf4d9d1089f80811cd6acad48e6b6a9d1f2003"
        ));

        assert_eq!(class_hash, expected_class_hash);

        let class_hash_in_cache = *db.db.contracts.get(&address).unwrap();
        let class_in_cache = db.db.classes.get(&class_hash).unwrap().class.clone();
        let storage_value_in_cache =
            *db.db.storage.get(&address).unwrap().storage.get(&storage_key).unwrap();

        assert_eq!(class_in_cache, class, "class must be stored in cache");
        assert_eq!(class_hash_in_cache, expected_class_hash, "class hash must be stored in cache");
        assert_eq!(storage_value_in_cache, storage_value, "storage value must be stored in cache");
    }
}

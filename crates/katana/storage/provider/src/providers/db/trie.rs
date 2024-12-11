use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;

use katana_db::abstraction::Database;
use katana_db::trie;
use katana_db::trie::{ContractTrie, StorageTrie};
use katana_primitives::block::BlockNumber;
use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_primitives::contract::StorageKey;
use katana_primitives::state::StateUpdates;
use katana_primitives::{ContractAddress, Felt};
use katana_trie::{compute_contract_state_hash, MultiProof};

use crate::providers::db::DbProvider;
use crate::traits::state::{StateFactoryProvider, StateProvider};
use crate::traits::trie::{
    ClassTrieProvider, ClassTrieWriter, ContractTrieProvider, ContractTrieWriter,
};
use crate::ProviderResult;

#[derive(Debug, Default)]
struct ContractLeaf {
    pub class_hash: Option<Felt>,
    pub storage_root: Option<Felt>,
    pub nonce: Option<Felt>,
}

impl<Db: Database> ContractTrieProvider for DbProvider<Db> {
    fn contract_trie_root(&self) -> ProviderResult<Felt> {
        Ok(trie::ContractTrie::new(self.0.tx_mut()?).root())
    }

    fn contracts_proof(
        &self,
        block_number: BlockNumber,
        contract_addresses: &[ContractAddress],
    ) -> ProviderResult<MultiProof> {
        let proofs = trie::ContractTrie::new_at_block(self.0.tx_mut()?, block_number)
            .expect("trie should exist")
            .get_multi_proof(contract_addresses);
        Ok(proofs)
    }

    fn storage_proof(
        &self,
        block_number: BlockNumber,
        contract_address: ContractAddress,
        storage_keys: Vec<StorageKey>,
    ) -> ProviderResult<MultiProof> {
        let proofs = trie::StorageTrie::new_at_block(self.0.tx_mut()?, block_number)
            .expect("trie should exist")
            .get_multi_proof(contract_address, storage_keys);
        Ok(proofs)
    }
}

impl<Db: Database> ClassTrieProvider for DbProvider<Db> {
    fn class_trie_root(&self) -> ProviderResult<Felt> {
        Ok(trie::ClassTrie::new(self.0.tx_mut()?).root())
    }

    fn classes_proof(
        &self,
        block_number: BlockNumber,
        class_hashes: &[ClassHash],
    ) -> ProviderResult<MultiProof> {
        let proofs = trie::ClassTrie::new_at_block(self.0.tx_mut()?, block_number)
            .expect("trie should exist")
            .get_multi_proof(class_hashes);
        Ok(proofs)
    }
}

impl<Db: Database> ClassTrieWriter for DbProvider<Db> {
    fn insert_updates(
        &self,
        block_number: BlockNumber,
        updates: &BTreeMap<ClassHash, CompiledClassHash>,
    ) -> ProviderResult<Felt> {
        let mut trie = trie::ClassTrie::new(self.0.tx_mut()?);

        for (class_hash, compiled_hash) in updates {
            trie.insert(*class_hash, *compiled_hash);
        }

        trie.commit(block_number);
        Ok(trie.root())
    }
}

impl<Db: Database> ContractTrieWriter for DbProvider<Db> {
    fn insert_updates(
        &self,
        block_number: BlockNumber,
        state_updates: &StateUpdates,
    ) -> ProviderResult<Felt> {
        let mut contract_leafs: HashMap<ContractAddress, ContractLeaf> = HashMap::new();

        let leaf_hashes: Vec<_> = {
            let mut storage_trie_db = StorageTrie::new(self.0.tx_mut()?);

            // First we insert the contract storage changes
            for (address, storage_entries) in &state_updates.storage_updates {
                for (key, value) in storage_entries {
                    storage_trie_db.insert(*address, *key, *value);
                }
                // insert the contract address in the contract_leafs to put the storage root later
                contract_leafs.insert(*address, Default::default());
            }

            // Then we commit them
            storage_trie_db.commit(block_number);

            for (address, nonce) in &state_updates.nonce_updates {
                contract_leafs.entry(*address).or_default().nonce = Some(*nonce);
            }

            for (address, class_hash) in &state_updates.deployed_contracts {
                contract_leafs.entry(*address).or_default().class_hash = Some(*class_hash);
            }

            for (address, class_hash) in &state_updates.replaced_classes {
                contract_leafs.entry(*address).or_default().class_hash = Some(*class_hash);
            }

            contract_leafs
                .into_iter()
                .map(|(address, mut leaf)| {
                    let storage_root = storage_trie_db.root(&address);
                    leaf.storage_root = Some(storage_root);

                    let latest_state = self.latest().unwrap();
                    let leaf_hash = contract_state_leaf_hash(latest_state, &address, &leaf);

                    (address, leaf_hash)
                })
                .collect::<Vec<_>>()
        };

        let mut contract_trie_db = ContractTrie::new(self.0.tx_mut()?);

        for (k, v) in leaf_hashes {
            contract_trie_db.insert(k, v);
        }

        contract_trie_db.commit(block_number);
        Ok(contract_trie_db.root())
    }
}

// computes the contract state leaf hash
fn contract_state_leaf_hash(
    provider: impl StateProvider,
    address: &ContractAddress,
    contract_leaf: &ContractLeaf,
) -> Felt {
    let nonce =
        contract_leaf.nonce.unwrap_or(provider.nonce(*address).unwrap().unwrap_or_default());

    let class_hash = contract_leaf
        .class_hash
        .unwrap_or(provider.class_hash_of_contract(*address).unwrap().unwrap_or_default());

    let storage_root = contract_leaf.storage_root.expect("root need to set");

    compute_contract_state_hash(&class_hash, &storage_root, &nonce)
}

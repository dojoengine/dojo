use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;

use bitvec::order::Msb0;
use bitvec::vec::BitVec;
use bitvec::view::AsBits;
use katana_db::abstraction::Database;
use katana_db::tables;
use katana_db::trie::TrieDb;
use katana_primitives::block::BlockNumber;
use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_primitives::state::StateUpdates;
use katana_primitives::{ContractAddress, Felt};
use katana_trie::bonsai::id::BasicId;
use katana_trie::bonsai::{BonsaiStorage, BonsaiStorageConfig};
use starknet_types_core::hash::{Pedersen, Poseidon, StarkHash};

use crate::providers::db::DbProvider;
use crate::traits::state::{StateFactoryProvider, StateProvider};
use crate::traits::trie::{ClassTrieWriter, ContractTrieWriter};

#[derive(Debug, Default)]
struct ContractLeaf {
    pub class_hash: Option<Felt>,
    pub storage_root: Option<Felt>,
    pub nonce: Option<Felt>,
}

impl<Db: Database> ClassTrieWriter for DbProvider<Db> {
    fn insert_updates(
        &self,
        block_number: BlockNumber,
        updates: &BTreeMap<ClassHash, CompiledClassHash>,
    ) -> crate::ProviderResult<Felt> {
        let db = TrieDb::<tables::ClassTrie, <Db as Database>::TxMut>::new(self.0.tx_mut()?);
        let mut trie = katana_trie::ClassTrie::new(db);
        let new_root = trie.apply(block_number, updates);
        Ok(new_root)
    }
}

impl<Db: Database> ContractTrieWriter for DbProvider<Db> {
    fn insert_updates(
        &self,
        block_number: BlockNumber,
        state_updates: &StateUpdates,
    ) -> crate::ProviderResult<Felt> {
        let config = BonsaiStorageConfig {
            max_saved_trie_logs: Some(0),
            max_saved_snapshots: Some(0),
            snapshot_interval: u64::MAX,
        };

        let mut contract_leafs: HashMap<ContractAddress, ContractLeaf> = HashMap::new();

        let leaf_hashes: Vec<_> = {
            let tx = self.0.tx_mut()?;
            let db = TrieDb::<tables::ContractStorageTrie, <Db as Database>::TxMut>::new(tx);
            let mut storage_trie_db =
                BonsaiStorage::<BasicId, _, Poseidon>::new(db, config.clone()).unwrap();

            // First we insert the contract storage changes
            for (address, storage_entries) in &state_updates.storage_updates {
                for (key, value) in storage_entries {
                    let keys = key.to_bytes_be();
                    let keys: BitVec<u8, Msb0> = keys.as_bits().to_owned();
                    storage_trie_db.insert(&address.to_bytes_be(), &keys, value).unwrap();
                }
                // insert the contract address in the contract_leafs to put the storage root later
                contract_leafs.insert(*address, Default::default());
            }

            // Then we commit them
            storage_trie_db.commit(BasicId::new(block_number)).unwrap();

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
                    let storage_root = storage_trie_db.root_hash(&address.to_bytes_be()).unwrap();
                    leaf.storage_root = Some(storage_root);

                    let latest_state = self.latest().unwrap();
                    let leaf_hash = contract_state_leaf_hash(latest_state, &address, &leaf);
                    let key: BitVec<u8, Msb0> = address.to_bytes_be().as_bits()[5..].to_owned();

                    (key, leaf_hash)
                })
                .collect::<Vec<_>>()
        };

        const IDENTIFIER: &[u8] = b"0xcontract";

        let tx = self.0.tx_mut()?;
        let db = TrieDb::<tables::ContractTrie, <Db as Database>::TxMut>::new(tx);
        let mut contract_trie_db =
            BonsaiStorage::<BasicId, _, Poseidon>::new(db, config.clone()).unwrap();

        for (k, v) in leaf_hashes {
            contract_trie_db.insert(IDENTIFIER, &k, &v).unwrap();
        }

        contract_trie_db.commit(BasicId::new(block_number)).unwrap();
        let root_hash = contract_trie_db.root_hash(IDENTIFIER).unwrap();

        Ok(root_hash)
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

    // hPed(
    //   hPed(
    //     hPed(
    //       class_hash,
    //       storage_root
    //     ),
    //     nonce
    //   ),
    //   0
    // )
    Pedersen::hash(
        &Pedersen::hash(&Pedersen::hash(&class_hash, &storage_root), &nonce),
        &Felt::ZERO,
    )
}

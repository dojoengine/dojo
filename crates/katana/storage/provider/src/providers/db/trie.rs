use std::collections::BTreeMap;

use katana_db::abstraction::Database;
use katana_db::tables;
use katana_db::trie::TrieDb;
use katana_primitives::block::BlockNumber;
use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_primitives::Felt;

use crate::providers::db::DbProvider;
use crate::traits::trie::{ClassTrieWriter, ContractTrieWriter};

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
    fn insert_updates(&self) -> crate::ProviderResult<Felt> {
        Ok(Felt::ZERO)
    }
}

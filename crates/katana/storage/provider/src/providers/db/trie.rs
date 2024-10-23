use std::collections::BTreeMap;

use katana_db::abstraction::Database;
use katana_db::trie::TrieDb;
use katana_primitives::block::BlockNumber;
use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_trie::class::ClassTrie;

use crate::providers::db::DbProvider;
use crate::traits::trie::{ClassTrieWriter, ContractTrieWriter};

impl<Db: Database> ClassTrieWriter for DbProvider<Db> {
    fn insert_updates(
        &self,
        block_number: BlockNumber,
        updates: &BTreeMap<ClassHash, CompiledClassHash>,
    ) -> crate::ProviderResult<()> {
        let tx = self.0.tx_mut()?;
        let mut trie = ClassTrie::new(TrieDb::new(tx));
        trie.apply(block_number, updates);
        Ok(())
    }
}

impl<Db: Database> ContractTrieWriter for DbProvider<Db> {
    fn insert_updates(&self) -> crate::ProviderResult<()> {
        Ok(())
    }
}

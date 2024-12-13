use std::collections::BTreeMap;

use katana_primitives::block::BlockNumber;
use katana_primitives::class::{ClassHash, CompiledClassHash};
use katana_primitives::contract::StorageKey;
use katana_primitives::state::StateUpdates;
use katana_primitives::{ContractAddress, Felt};
use katana_trie::MultiProof;

use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateRootProvider: Send + Sync {
    fn state_root(&self) -> ProviderResult<Felt>;

    fn classes_root(&self) -> ProviderResult<Felt>;

    fn contracts_root(&self) -> ProviderResult<Felt>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ClassTrieProvider: Send + Sync {
    fn classes_proof(
        &self,
        block_number: BlockNumber,
        class_hashes: &[ClassHash],
    ) -> ProviderResult<MultiProof>;

    fn class_trie_root(&self) -> ProviderResult<Felt>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractTrieProvider: Send + Sync {
    fn contracts_proof(
        &self,
        block_number: BlockNumber,
        contract_addresses: &[ContractAddress],
    ) -> ProviderResult<MultiProof>;

    fn storage_proof(
        &self,
        block_number: BlockNumber,
        contract_address: ContractAddress,
        storage_keys: Vec<StorageKey>,
    ) -> ProviderResult<MultiProof>;

    fn contract_trie_root(&self) -> ProviderResult<Felt>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ClassTrieWriter: Send + Sync {
    fn insert_updates(
        &self,
        block_number: BlockNumber,
        updates: &BTreeMap<ClassHash, CompiledClassHash>,
    ) -> ProviderResult<Felt>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractTrieWriter: Send + Sync {
    fn insert_updates(
        &self,
        block_number: BlockNumber,
        state_updates: &StateUpdates,
    ) -> ProviderResult<Felt>;
}

use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::class::ClassHash;
use katana_primitives::contract::{ContractAddress, Nonce, StorageKey, StorageValue};
use katana_primitives::FieldElement;

use super::contract::ContractClassProvider;
use crate::ProviderResult;

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateRootProvider: Send + Sync {
    /// Retrieves the state root of a block.
    fn state_root(&self, block_id: BlockHashOrNumber) -> ProviderResult<Option<FieldElement>>;
}

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateProvider: ContractClassProvider + Send + Sync {
    /// Returns the nonce of a contract.
    fn nonce(&self, address: ContractAddress) -> ProviderResult<Option<Nonce>>;

    /// Returns the value of a contract storage.
    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> ProviderResult<Option<StorageValue>>;

    /// Returns the class hash of a contract.
    fn class_hash_of_contract(&self, address: ContractAddress)
    -> ProviderResult<Option<ClassHash>>;
}

/// A type which can create [`StateProvider`] for states at a particular block.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateFactoryProvider: Send + Sync {
    /// Returns a state provider for retrieving the latest state.
    fn latest(&self) -> ProviderResult<Box<dyn StateProvider>>;

    /// Returns a state provider for retrieving historical state at the given block.
    fn historical(
        &self,
        block_id: BlockHashOrNumber,
    ) -> ProviderResult<Option<Box<dyn StateProvider>>>;
}

// TEMP: added mainly for compatibility reason. it might be removed in the future.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateWriter: Send + Sync {
    /// Sets the nonce of a contract.
    fn set_nonce(&self, address: ContractAddress, nonce: Nonce) -> ProviderResult<()>;

    /// Sets the value of a contract storage.
    fn set_storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
        storage_value: StorageValue,
    ) -> ProviderResult<()>;

    /// Sets the class hash of a contract.
    fn set_class_hash_of_contract(
        &self,
        address: ContractAddress,
        class_hash: ClassHash,
    ) -> ProviderResult<()>;
}

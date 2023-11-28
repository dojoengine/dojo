use anyhow::Result;
use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::contract::{ClassHash, ContractAddress, Nonce, StorageKey, StorageValue};

use super::contract::{ContractClassProvider, ContractClassWriter, ContractInfoProvider};

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateProvider: ContractInfoProvider + ContractClassProvider + Send + Sync {
    /// Returns the nonce of a contract.
    fn nonce(&self, address: ContractAddress) -> Result<Option<Nonce>>;

    /// Returns the value of a contract storage.
    fn storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
    ) -> Result<Option<StorageValue>>;

    /// Returns the class hash of a contract.
    fn class_hash_of_contract(&self, address: ContractAddress) -> Result<Option<ClassHash>>;
}

/// A state factory provider is a provider which can create state providers for
/// states at a particular block.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateFactoryProvider {
    /// Returns a state provider for retrieving the latest state.
    fn latest(&self) -> Result<Box<dyn StateProvider>>;

    /// Returns a state provider for retrieving historical state at the given block.
    fn historical(&self, block_id: BlockHashOrNumber) -> Result<Option<Box<dyn StateProvider>>>;
}

// TEMP: added mainly for compatibility reason following the path of least resistance.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateWriter: StateProvider + ContractClassWriter + Send + Sync {
    /// Sets the nonce of a contract.
    fn set_nonce(&self, address: ContractAddress, nonce: Nonce) -> Result<()>;

    /// Sets the value of a contract storage.
    fn set_storage(
        &self,
        address: ContractAddress,
        storage_key: StorageKey,
        storage_value: StorageValue,
    ) -> Result<()>;

    /// Sets the class hash of a contract.
    fn set_class_hash_of_contract(
        &self,
        address: ContractAddress,
        class_hash: ClassHash,
    ) -> Result<()>;
}

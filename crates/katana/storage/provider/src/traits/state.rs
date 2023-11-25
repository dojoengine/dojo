use anyhow::Result;
use katana_primitives::block::BlockHashOrNumber;
use katana_primitives::contract::{
    ClassHash, CompiledClassHash, CompiledContractClass, ContractAddress, Nonce, SierraClass,
    StorageKey, StorageValue,
};

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateProvider: Send + Sync {
    /// Returns the compiled class definition of a contract class given its class hash.
    fn class(&self, hash: ClassHash) -> Result<Option<CompiledContractClass>>;

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

/// An extension of the `StateProvider` trait which provides additional methods.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait StateProviderExt: StateProvider + Send + Sync {
    /// Retrieves the Sierra class definition of a contract class given its class hash.
    fn sierra_class(&self, hash: ClassHash) -> Result<Option<SierraClass>>;

    /// Returns the compiled class hash for the given class hash.
    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> Result<Option<CompiledClassHash>>;
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

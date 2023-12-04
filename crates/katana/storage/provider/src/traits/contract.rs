use anyhow::Result;
use katana_primitives::contract::{
    ClassHash, CompiledClassHash, CompiledContractClass, ContractAddress, GenericContractInfo,
    SierraClass,
};

#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractInfoProvider: Send + Sync {
    /// Returns the contract information given its address.
    fn contract(&self, address: ContractAddress) -> Result<Option<GenericContractInfo>>;
}

/// A provider trait for retrieving contract class related information.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractClassProvider: Send + Sync {
    /// Returns the compiled class hash for the given class hash.
    fn compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
    ) -> Result<Option<CompiledClassHash>>;

    /// Returns the compiled class definition of a contract class given its class hash.
    fn class(&self, hash: ClassHash) -> Result<Option<CompiledContractClass>>;

    /// Retrieves the Sierra class definition of a contract class given its class hash.
    fn sierra_class(&self, hash: ClassHash) -> Result<Option<SierraClass>>;
}

// TEMP: added mainly for compatibility reason. might be removed in the future.
#[auto_impl::auto_impl(&, Box, Arc)]
pub trait ContractClassWriter: Send + Sync {
    /// Returns the compiled class hash for the given class hash.
    fn set_compiled_class_hash_of_class_hash(
        &self,
        hash: ClassHash,
        compiled_hash: CompiledClassHash,
    ) -> Result<()>;

    /// Returns the compiled class definition of a contract class given its class hash.
    fn set_class(&self, hash: ClassHash, class: CompiledContractClass) -> Result<()>;

    /// Retrieves the Sierra class definition of a contract class given its class hash.
    fn set_sierra_class(&self, hash: ClassHash, sierra: SierraClass) -> Result<()>;
}

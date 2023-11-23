use anyhow::Result;
use katana_primitives::contract::{ContractAddress, GenericContractInfo};

pub trait ContractProvider: Send + Sync {
    /// Returns the contract information given its address.
    fn contract(&self, address: ContractAddress) -> Result<Option<GenericContractInfo>>;
}

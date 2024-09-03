use starknet::{ClassHash, ContractAddress};

#[derive(Drop, Serde, Debug)]
pub struct ContractDefinition {
    pub class_hash: ClassHash,
    pub contract_address: ContractAddress
}

#[starknet::interface]
pub trait IContract<T> {
    fn contract_name(self: @T) -> ByteArray;
    fn namespace(self: @T) -> ByteArray;
    fn tag(self: @T) -> ByteArray;

    fn name_hash(self: @T) -> felt252;
    fn namespace_hash(self: @T) -> felt252;
    fn selector(self: @T) -> felt252;
}

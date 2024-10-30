//! ResourceMetadata model.
//!
use dojo::model::model::Model;
use dojo::utils;

#[derive(Introspect, Drop, Serde, PartialEq, Clone, Debug)]
#[dojo::model]
pub struct ResourceMetadata {
    #[key]
    pub resource_id: felt252,
    pub metadata_uri: ByteArray,
}

pub fn default_address() -> starknet::ContractAddress {
    starknet::contract_address_const::<0>()
}

pub fn default_class_hash() -> starknet::ClassHash {
    starknet::class_hash::class_hash_const::<0>()
}

pub fn resource_metadata_selector(default_namespace_hash: felt252) -> felt252 {
    utils::selector_from_namespace_and_name(
        default_namespace_hash, @Model::<ResourceMetadata>::name()
    )
}

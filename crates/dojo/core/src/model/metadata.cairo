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
    pub metadata_hash: felt252,
}

pub fn default_address() -> starknet::ContractAddress {
    0.try_into().unwrap()
}

pub fn default_class_hash() -> starknet::ClassHash {
    0.try_into().unwrap()
}

pub fn resource_metadata_selector(default_namespace_hash: felt252) -> felt252 {
    utils::selector_from_namespace_and_name(
        default_namespace_hash, @Model::<ResourceMetadata>::name(),
    )
}

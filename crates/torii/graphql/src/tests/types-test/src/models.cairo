use array::ArrayTrait;
use starknet::{ContractAddress, ClassHash};

#[derive(Model, Copy, Drop, Serde)]
struct Record {
    #[key]
    record_id: u32,
    type_u8: u8,
    type_u16: u16,
    type_u32: u32,
    type_u64: u64,
    type_u128: u128,
    type_u256: u256,
    type_bool: bool,
    type_felt: felt252,
    type_class_hash: ClassHash,
    type_contract_address: ContractAddress,
    type_nested: Nested,
}

#[derive(Copy, Drop, Serde, Introspect)]
struct Nested {
    depth: u8,
    type_number: u8,
    type_string: felt252,
    type_nested_more: NestedMore,
}

#[derive(Copy, Drop, Serde, Introspect)]
struct NestedMore {
    depth: u8,
    type_number: u8,
    type_string: felt252,
    type_nested_more_more: NestedMoreMore,
}

#[derive(Copy, Drop, Serde, Introspect)]
struct NestedMoreMore {
    depth: u8,
    type_number: u8,
    type_string: felt252,
}

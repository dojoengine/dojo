use starknet::ContractAddress;
use dojo_erc::erc_common::components::{operator_approval, OperatorApproval};

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Balance {
    #[key]
    token: ContractAddress,
    #[key]
    account: ContractAddress,
    amount: u128,
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Owner {
    #[key]
    token: ContractAddress,
    #[key]
    token_id: felt252,
    address: ContractAddress
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct TokenApproval {
    #[key]
    token: ContractAddress,
    #[key]
    token_id: felt252,
    address: ContractAddress,
}



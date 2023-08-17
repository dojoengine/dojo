use starknet::ContractAddress;
use dojo_erc::erc_common::components::{operator_approval, OperatorApproval};

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Uri {
    #[key]
    token: ContractAddress,
    uri: felt252
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct ERC1155Balance {
    #[key]
    token: ContractAddress,
    #[key]
    token_id: felt252,
    #[key]
    account: ContractAddress,

    amount: u128
}

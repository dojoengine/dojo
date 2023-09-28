use starknet::ContractAddress;

#[derive(Model, Copy, Drop, Serde)]
struct ERC721Meta {
    #[key]
    token: ContractAddress,
    name: felt252,
    symbol: felt252,
    base_uri: felt252,
}

#[derive(Model, Copy, Drop, Serde)]
struct ERC721OperatorApproval {
    #[key]
    token: ContractAddress,
    #[key]
    owner: ContractAddress,
    #[key]
    operator: ContractAddress,
    approved: bool
}

#[derive(Model, Copy, Drop, Serde)]
struct ERC721Owner {
    #[key]
    token: ContractAddress,
    #[key]
    token_id: u256,
    address: ContractAddress
}

#[derive(Model, Copy, Drop, Serde)]
struct ERC721Balance {
    #[key]
    token: ContractAddress,
    #[key]
    account: ContractAddress,
    amount: u256,
}

#[derive(Model, Copy, Drop, Serde)]
struct ERC721TokenApproval {
    #[key]
    token: ContractAddress,
    #[key]
    token_id: u256,
    address: ContractAddress,
}
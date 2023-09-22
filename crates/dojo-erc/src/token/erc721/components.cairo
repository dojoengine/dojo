use starknet::ContractAddress;

#[derive(Component, Copy, Drop, Serde)]
struct ERC721Meta {
    #[key]
    token: ContractAddress,
    name: felt252,
    symbol: felt252,
    base_uri: felt252,
}

#[derive(Component, Copy, Drop, Serde)]
struct ERC721OperatorApproval {
    #[key]
    token: ContractAddress,
    #[key]
    owner: ContractAddress,
    #[key]
    operator: ContractAddress,
    approved: bool
}

#[derive(Component, Copy, Drop, Serde)]
struct ERC721Owner {
    #[key]
    token: ContractAddress,
    #[key]
    token_id: u256,
    address: ContractAddress
}

#[derive(Component, Copy, Drop, Serde)]
struct ERC721Balance {
    #[key]
    token: ContractAddress,
    #[key]
    account: ContractAddress,
    amount: u256,
}

#[derive(Component, Copy, Drop, Serde)]
struct ERC721TokenApproval {
    #[key]
    token: ContractAddress,
    #[key]
    token_id: u256,
    address: ContractAddress,
}
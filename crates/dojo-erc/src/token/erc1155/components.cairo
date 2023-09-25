use starknet::ContractAddress;

#[derive(Component, Copy, Drop, Serde)]
struct ERC1155Meta {
    #[key]
    token: ContractAddress,
    name: felt252,
    symbol: felt252,
    base_uri: felt252,
}

#[derive(Component, Copy, Drop, Serde)]
struct ERC1155OperatorApproval {
    #[key]
    token: ContractAddress,
    #[key]
    owner: ContractAddress,
    #[key]
    operator: ContractAddress,
    approved: bool
}


#[derive(Component, Copy, Drop, Serde)]
struct ERC1155Balance {
    #[key]
    token: ContractAddress,
    #[key]
    account: ContractAddress,
    #[key]
    id: u256,
    amount: u256
}
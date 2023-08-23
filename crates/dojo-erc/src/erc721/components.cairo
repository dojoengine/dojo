use starknet::ContractAddress;

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Balance {
    #[key]
    token: ContractAddress,
    #[key]
    account: ContractAddress,
    amount: felt252,
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

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct OperatorApproval {
    #[key]
    token: ContractAddress,
    #[key]
    owner: ContractAddress,
    #[key]
    operator: ContractAddress,
    approved: bool
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct TokenUri {
    #[key]
    token_id: felt252,
    uri: felt252
}

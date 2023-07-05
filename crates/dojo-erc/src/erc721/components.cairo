use starknet::ContractAddress;

#[derive(Component, Copy, Drop, Serde)]
struct Balances {
    amount: felt252, 
}

#[derive(Component, Copy, Drop, Serde)]
struct Owners {
    address: felt252
}

#[derive(Component, Copy, Drop, Serde)]
struct TokenApprovals {
    address: felt252, 
}

#[derive(Component, Copy, Drop, Serde)]
struct OperatorApprovals {
    approved: felt252
}

#[derive(Component, Copy, Drop, Serde)]
struct TokenUri {
    uri: felt252
}

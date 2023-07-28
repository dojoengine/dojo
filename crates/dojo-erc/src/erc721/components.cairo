use starknet::ContractAddress;

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Balances {
    amount: felt252, 
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Owners {
    address: felt252
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct TokenApprovals {
    address: felt252, 
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct OperatorApprovals {
    approved: felt252
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct TokenUri {
    uri: felt252
}

use starknet::ContractAddress;

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Allowance {
    #[key]
    token: ContractAddress,
    #[key]
    owner: ContractAddress,
    #[key]
    spender: ContractAddress,
    amount: felt252,
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Balance {
    #[key]
    token: ContractAddress,
    #[key]
    sender: ContractAddress,
    amount: felt252,
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Supply {
    #[key]
    token: ContractAddress,
    amount: felt252
}
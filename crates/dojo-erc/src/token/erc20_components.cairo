use starknet::ContractAddress;

#[derive(Model, Copy, Drop, Serde)]
struct ERC20Balance {
    #[key]
    token: ContractAddress,
    #[key]
    account: ContractAddress,
    amount: u256,
}

#[derive(Model, Copy, Drop, Serde)]
struct ERC20Allowance {
    #[key]
    token: ContractAddress,
    #[key]
    owner: ContractAddress,
    #[key]
    spender: ContractAddress,
    amount: u256,
}

#[derive(Model, Copy, Drop, Serde)]
struct ERC20Meta {
    #[key]
    token: ContractAddress,
    name: felt252,
    symbol: felt252,
    total_supply: u256,
}

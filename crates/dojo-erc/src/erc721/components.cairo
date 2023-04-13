use starknet::ContractAddress;

#[derive(Component)]
struct Balance {
    value: u64
}

#[derive(Component)]
struct Owner {
    address: ContractAddress
}

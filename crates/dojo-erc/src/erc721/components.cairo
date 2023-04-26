use starknet::ContractAddress;

#[derive(Component]
struct TokenApproval {
    address: ContractAddress
}

#[derive(Component)]
struct OperatorApproval {
    value: bool
}

#[derive(Component)]
struct Balance {
    value: u128
}

#[derive(Component)]
struct Owner {
    address: ContractAddress
}

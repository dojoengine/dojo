#[derive(Component, Copy, Drop, Serde)]
struct OperatorApproval {
    value: bool
}

#[derive(Component, Copy, Drop, Serde)]
struct Balance {
    amount: felt252
}

#[derive(Component, Copy, Drop, Serde)]
struct Uri {
    uri: felt252
}

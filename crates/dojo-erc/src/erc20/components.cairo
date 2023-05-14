#[derive(Component, Copy, Drop, Serde)]
struct Approval {
    amount: felt252,
}

#[derive(Component, Copy, Drop, Serde)]
struct Ownership {
    balance: felt252,
}

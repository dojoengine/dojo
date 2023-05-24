#[derive(Component, Copy, Drop, Serde)]
struct Allowance {
    amount: felt252,
}

#[derive(Component, Copy, Drop, Serde)]
struct Balance {
    amount: felt252,
}

#[derive(Component, Copy, Drop, Serde)]
struct Supply {
    amount: felt252
}

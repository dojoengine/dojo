#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Allowance {
    amount: felt252, 
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Balance {
    amount: felt252, 
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Supply {
    amount: felt252
}

use option::OptionTrait;
use starknet::ContractAddress;
use traits::{Into, TryInto};

use dojo::serde::SerdeLen;


// Cubit fixed point math library
use cubit::f128::types::fixed::Fixed;

const SCALING_FACTOR: u128 = 10000;

impl SerdeLenFixed of SerdeLen<Fixed> {
    #[inline(always)]
    fn len() -> usize {
        2
    }
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Cash {
    #[key]
    player: ContractAddress,
    amount: u128,
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Item {
    #[key]
    player: ContractAddress,
    #[key]
    item_id: u32,
    quantity: u128,
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Liquidity {
    #[key]
    player: ContractAddress,
    #[key]
    item_id: u32,
    shares: Fixed,
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Market {
    #[key]
    item_id: u32,
    cash_amount: u128,
    item_quantity: u128,
}

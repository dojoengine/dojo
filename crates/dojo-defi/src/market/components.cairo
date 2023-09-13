use starknet::ContractAddress;
use dojo::StorageSize;

// Cubit fixed point math library
use cubit::f128::types::fixed::Fixed;

const SCALING_FACTOR: u128 = 10000;

impl StorageSizeFixed of StorageSize<Fixed> {
    #[inline(always)]
    fn unpacked_size() -> usize {
        1
    }

    #[inline(always)]
    fn packed_size() -> usize {
        129
    }
}

#[derive(Component, Copy, Drop, Serde)]
struct Cash {
    #[key]
    player: ContractAddress,
    amount: u128,
}

#[derive(Component, Copy, Drop, Serde)]
struct Item {
    #[key]
    player: ContractAddress,
    #[key]
    item_id: u32,
    quantity: u128,
}

#[derive(Component, Copy, Drop, Serde)]
struct Liquidity {
    #[key]
    player: ContractAddress,
    #[key]
    item_id: u32,
    shares: Fixed,
}

#[derive(Component, Copy, Drop, Serde)]
struct Market {
    #[key]
    item_id: u32,
    cash_amount: u128,
    item_quantity: u128,
}

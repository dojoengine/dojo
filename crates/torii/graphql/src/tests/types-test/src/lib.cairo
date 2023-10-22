mod models;
mod contracts;


fn seed() -> felt252 {
    starknet::get_tx_info().unbox().transaction_hash
}

fn random(seed: felt252, min: u128, max: u128) -> u128 {
    let seed: u256 = seed.into();
    let range = max - min;

    (seed.low % range) + min
}

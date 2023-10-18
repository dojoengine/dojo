use core::result::ResultTrait;
use array::ArrayTrait;
use array::SpanTrait;
use debug::PrintTrait;

use dojo::database::storage;
use dojo::packing::{shl, shr};

const GAS_OFFSET: felt252 = 0x1_000000_000000_000000_000000_000000; // 15 bajtów

fn start() -> u128 {
    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    gas
}

fn end(start: u128, name: felt252) {
    let gas_after = testing::get_available_gas();
    let mut name: u256 = name.into();

    // overwriting zeros with spaces
    let mut char = 0;
    loop {
        if char == 15 {
            break;
        }
        // if given byte is zero
        if shl(0xff, 8 * char) & name == 0 {
            name = name | shl(0x20, 8 * char); // set space
        }
        char += 1;
    };

    let name: felt252 = (name % GAS_OFFSET.into()).try_into().unwrap();
    let used_gas = (start - gas_after).into() * GAS_OFFSET;
    (used_gas + name).print();
}


#[test]
#[available_gas(3000000)]
fn bench_storage_single() {
    let keys = array!['database_test', '42'].span();

    storage::set(0, keys, 420);

    let res = storage::get(0, keys);
    assert(res == 420, 'values differ');
}

#[test]
#[available_gas(10000000)]
fn bench_storage_many() {
    let keys = array![0x1337].span();
    let values = array![1, 2].span();
    let layout = array![251, 251].span();

    let time = start();
    storage::set_many(0, keys, 0, values, layout);
    end(time, 'storage many');

    let res = storage::get_many(0, keys, 0, 2, layout);
    assert(res.len() == 2, 'wrong number of values');
    assert(*res.at(0) == *values.at(0), 'value not set');
    assert(*res.at(1) == *values.at(1), 'value not set');
}
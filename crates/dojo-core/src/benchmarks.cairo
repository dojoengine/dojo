use core::result::ResultTrait;
use array::ArrayTrait;
use array::SpanTrait;
use debug::PrintTrait;
use option::OptionTrait;

use dojo::database::storage;
use dojo::database::index;
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
#[available_gas(1000000000)]
fn bench_reference_offset() {
    let time = start();
    end(time, 'bench empty');
}

#[test]
#[available_gas(1000000000)]
fn bench_storage_single() {
    let keys = array!['database_test', '42'].span();

    let time = start();
    storage::set(0, keys, 420);
    end(time, 'storage set');

    let time = start();
    let res = storage::get(0, keys);
    end(time, 'storage get');

    assert(res == 420, 'values differ');
}

#[test]
#[available_gas(1000000000)]
fn bench_storage_many() {
    let keys = array![0x1337].span();
    let values = array![1, 2].span();
    let layout = array![251, 251].span();

    let time = start();
    storage::set_many(0, keys, 0, values, layout);
    end(time, 'storage set mny');

    let time = start();
    let res = storage::get_many(0, keys, 0, 2, layout);
    end(time, 'storage get mny');

    assert(res.len() == 2, 'wrong number of values');
    assert(*res.at(0) == *values.at(0), 'value not set');
    assert(*res.at(1) == *values.at(1), 'value not set');
}

#[test]
#[available_gas(1000000000)]
fn bench_index() {
    let time = start();
    let no_query = index::query(0, 69, Option::None(()));
    end(time, 'idx empty');
    assert(no_query.len() == 0, 'entity indexed');

    let time = start();
    index::create(0, 69, 420);
    end(time, 'idx create 1st');

    let time = start();
    let query = index::query(0, 69, Option::None(()));
    end(time, 'idx query one');
    assert(query.len() == 1, 'entity not indexed');
    assert(*query.at(0) == 420, 'entity value incorrect');
    
    let time = start();
    index::create(0, 69, 1337);
    end(time, 'idx query 2nd');

    let time = start();
    let two_query = index::query(0, 69, Option::None(()));
    end(time, 'idx query two');
    assert(two_query.len() == 2, 'index should have two query');
    assert(*two_query.at(1) == 1337, 'entity value incorrect');

    let time = start();
    index::exists(0, 69, 420);
    end(time, 'idx exists chk');

    let time = start();
    index::delete(0, 69, 420);
    end(time, 'idx dlt !last');

    assert(!index::exists(0, 69, 420), 'entity should not exist');

    let time = start();
    index::delete(0, 69, 1337);
    end(time, 'idx dlt last');

    assert(!index::exists(0, 69, 1337), 'entity should not exist');
}
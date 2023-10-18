use core::result::ResultTrait;
use array::ArrayTrait;
use array::SpanTrait;
use debug::PrintTrait;

use dojo::database::storage;


fn start() -> u128 {
    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    gas
}

fn end(start: u128, name: felt252) {
    // assert(name < 16777216 * 16777216, "name would not fit"); 
    let gas_after = testing::get_available_gas();
    let used_gas: felt252 = (start - gas_after).into();
    let used_gas = used_gas * 16777216 * 16777216 * 256;
    let delimiter = 'tt' * 16777216 * 16777216;
    
    (used_gas + delimiter + name).print();
}


#[test]
#[available_gas(2000000)]
fn bench_storage_single() {
    let keys = array!['database_test', '42'].span();

    storage::set(0, keys, 420);

    let res = storage::get(0, keys);
    assert(res == 420, 'values differ');
}

#[test]
#[available_gas(2000000)]
fn bench_storage_many() {
    let keys = array![0x1337].span();
    let values = array![1, 2].span();
    let layout = array![251, 251].span();



    let time = start();

    storage::set_many(0, keys, 0, values, layout);

    end(time, 'strg_m');

    let res = storage::get_many(0, keys, 0, 2, layout);
    assert(res.len() == 2, 'wrong number of values');
    assert(*res.at(0) == *values.at(0), 'value not set');
    assert(*res.at(1) == *values.at(1), 'value not set');
}
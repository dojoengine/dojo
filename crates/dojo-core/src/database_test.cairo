use core::result::ResultTrait;
use array::ArrayTrait;
use option::OptionTrait;
use serde::Serde;
use array::SpanTrait;
use traits::{Into, TryInto};

use starknet::syscalls::deploy_syscall;
use starknet::class_hash::{Felt252TryIntoClassHash, ClassHash};
use dojo::world::{IWorldDispatcher};
use dojo::executor::executor;
use dojo::database::{get, set, set_with_index, del, scan, QueryClause, KeyValueClause};

#[test]
#[available_gas(1000000)]
fn test_database_basic() {
    let mut values = ArrayTrait::new();
    values.append('database_test');
    values.append('42');

    set('table', 'key', 0, values.span(), array![251, 251].span());
    let res = get('table', 'key', 0, values.len(), array![251, 251].span());

    assert(res.at(0) == values.at(0), 'Value at 0 not equal!');
    assert(res.at(1) == values.at(1), 'Value at 0 not equal!');
    assert(res.len() == values.len(), 'Lengths not equal');
}

#[test]
#[available_gas(1500000)]
fn test_database_different_tables() {
    let mut values = ArrayTrait::new();
    values.append(0x1);
    values.append(0x2);

    let mut other = ArrayTrait::new();
    other.append(0x3);
    other.append(0x4);

    set('first', 'key', 0, values.span(), array![251, 251].span());
    set('second', 'key', 0, other.span(), array![251, 251].span());
    let res = get('first', 'key', 0, values.len(), array![251, 251].span());
    let other_res = get('second', 'key', 0, other.len(), array![251, 251].span());

    assert(res.len() == values.len(), 'Lengths not equal');
    assert(res.at(0) == values.at(0), 'Values different at `first`!');
    assert(other_res.at(0) == other_res.at(0), 'Values different at `second`!');
    assert(other_res.at(0) != res.at(0), 'Values the same for different!');
}

#[test]
#[available_gas(1500000)]
fn test_database_different_keys() {
    let mut values = ArrayTrait::new();
    values.append(0x1);
    values.append(0x2);

    let mut other = ArrayTrait::new();
    other.append(0x3);
    other.append(0x4);

    set('table', 'key', 0, values.span(), array![251, 251].span());
    set('table', 'other', 0, other.span(), array![251, 251].span());
    let res = get('table', 'key', 0, values.len(), array![251, 251].span());
    let other_res = get('table', 'other', 0, other.len(), array![251, 251].span());

    assert(res.len() == values.len(), 'Lengths not equal');
    assert(res.at(0) == values.at(0), 'Values different at `key`!');
    assert(other_res.at(0) == other_res.at(0), 'Values different at `other`!');
    assert(other_res.at(0) != res.at(0), 'Values the same for different!');
}

#[test]
#[available_gas(10000000)]
fn test_database_pagination() {
    let mut values = ArrayTrait::new();
    values.append(0x1);
    values.append(0x2);
    values.append(0x3);
    values.append(0x4);
    values.append(0x5);

    set('table', 'key', 1, values.span(), array![251, 251, 251, 251, 251].span());
    let first_res = get('table', 'key', 1, 3, array![251, 251, 251].span());
    let second_res = get('table', 'key', 3, 5, array![251, 251, 251, 251, 251].span());
    let third_res = get('table', 'key', 5, 7, array![251, 251, 251, 251, 251, 251, 251].span());

    assert(*first_res.at(0) == *values.at(0), 'Values different at index 0!');
    assert(*first_res.at(1) == *values.at(1), 'Values different at index 1!');
    assert(*second_res.at(0) == *values.at(2), 'Values different at index 2!');
    assert(*second_res.at(1) == *values.at(3), 'Values different at index 3!');
    assert(*third_res.at(0) == *values.at(4), 'Values different at index 4!');
    assert(*third_res.at(1) == 0x0, 'Value not empty at index 5!');
}

#[test]
#[available_gas(10000000)]
fn test_database_del() {
    let mut values = ArrayTrait::new();
    values.append(0x42);

    set('table', 'key', 0, values.span(), array![251].span());

    let before = get('table', 'key', 0, values.len(), array![251].span());
    assert(*before.at(0) == *values.at(0), 'Values different at index 0!');

    del('table', 'key');
    let after = get('table', 'key', 0, 0, array![].span());
    assert(after.len() == 0, 'Non empty after deletion!');
}

#[test]
#[available_gas(10000000)]
fn test_database_scan() {
    let even = array![2, 4, 6].span();
    let odd = array![1, 3, 5].span();
    let layout = array![251, 251, 251].span();

    set_with_index('table', 'even', array!['x'].span(), 0, even, layout);
    set_with_index('table', 'odd', array!['x'].span(), 0, odd, layout);

    let (keys, values) = scan('table', Option::None, QueryClause::All, 4, layout);
    assert(keys.len() == 2, 'Wrong number of keys!');
    assert(values.len() == 2, 'Wrong number of values!');
    assert(*keys.at(0) == 'even', 'Wrong key at index 0!');
    assert(*(*values.at(0)).at(0) == 2, 'Wrong value at index 0!');
    assert(*(*values.at(0)).at(1) == 4, 'Wrong value at index 1!');
    assert(*(*values.at(0)).at(2) == 6, 'Wrong value at index 1!');

    let where = KeyValueClause {
        key: 0,
        value: 'x',
    };

    let (keys, values) = scan(class_hash, 'table', Option::None, QueryClause::KeyValue(where), 2, layout);
    assert(keys.len() == 2, 'Wrong number of keys clause!');
}

#[test]
#[available_gas(10000000)]
fn test_database_scan_where() {
    let some = array![2, 4].span();
    let same = array![1, 3].span();
    let other = array![5, 5].span();
    let layout = array![251, 251].span();

    let class_hash: starknet::ClassHash = executor::TEST_CLASS_HASH.try_into().unwrap();
    set_with_index(class_hash, 'table', 'some', array!['p'].span(), 0, some, layout);
    set_with_index(class_hash, 'table', 'same', array!['p'].span(), 0, same, layout);
    set_with_index(class_hash, 'table', 'other', array!['x'].span(), 0, other, layout);

    let (keys, values) = scan(class_hash, 'table', Option::None, QueryClause::All, 2, layout);
    assert(keys.len() == 3, 'Wrong number of keys!');
    assert(*(*values.at(0)).at(0) != 0, 'value is not set');

    let mut where = KeyValueClause {
        key: 0,
        value: 'x',
    };

    let (keys, values) = scan(class_hash, 'table', Option::None, QueryClause::KeyValue(where), 2, layout);
    assert(keys.len() == 1, 'Wrong number of keys for x!');
    assert(*(*values.at(0)).at(0) == 5, 'Wrong value 0 for x!!');
    assert(*(*values.at(0)).at(1) == 5, 'Wrong value 1 for x!');

    where.value = 'p';
    let (keys, values) = scan(class_hash, 'table', Option::None, QueryClause::KeyValue(where), 2, layout);
    assert(keys.len() == 2, 'Wrong number of keys for p!');

    let (keys, values) = scan(class_hash, 'table', Option::Some(1), QueryClause::KeyValue(where), 2, layout);
    assert(keys.len() == 1, 'Wrong len for second of p!');

    where.value = 'q';
    let (keys, values) = scan(class_hash, 'table', Option::None, QueryClause::KeyValue(where), 2, layout);
    assert(keys.len() == 0, 'Wrong number of keys for q!');
}

#[test]
#[available_gas(20000000)]
fn test_database_scan_where_deletion() {
    let some = array![2, 4].span();
    let same = array![1, 3].span();
    let other = array![5, 5].span();
    let layout = array![251, 251].span();

    let class_hash: starknet::ClassHash = executor::TEST_CLASS_HASH.try_into().unwrap();
    set_with_index(class_hash, 'table', 'some', array!['a', 'x'].span(), 0, some, layout);
    set_with_index(class_hash, 'table', 'same', array!['a', 'y'].span(), 0, same, layout);
    set_with_index(class_hash, 'table', 'other', array!['b', 'x'].span(), 0, other, layout);

    del(class_hash, 'table', 'same');

    let mut where = KeyValueClause {
        key: 0,
        value: 'a',
    };

    let (keys, values) = scan(class_hash, 'table', Option::None, QueryClause::KeyValue(where), 2, layout);
    assert(keys.len() == 1, 'Wrong number of keys for a!');
    assert(*(*values.at(0)).at(0) == 2, 'Wrong value 0 for a!');

    where.key = 1;
    where.value = 'y';
    let (keys, values) = scan(class_hash, 'table', Option::None, QueryClause::KeyValue(where), 2, layout);
    assert(keys.len() == 0, 'Wrong number of keys for y!');

    where.value = 'x';
    let (keys, values) = scan(class_hash, 'table', Option::None, QueryClause::KeyValue(where), 2, layout);
    assert(keys.len() == 2, 'Wrong number of keys for x!');

    del(class_hash, 'table', 'some');
    del(class_hash, 'table', 'other');

    let (keys, values) = scan(class_hash, 'table', Option::None, QueryClause::All, 2, layout);
    assert(keys.len() == 0, 'Wrong number of keys for c!');
}

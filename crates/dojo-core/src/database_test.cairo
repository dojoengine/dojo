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
use dojo::database::{get, set, del, all};

#[test]
#[available_gas(1000000)]
fn test_database_basic() {
    let mut values = ArrayTrait::new();
    values.append('database_test');
    values.append('42');

    let class_hash: starknet::ClassHash = executor::TEST_CLASS_HASH.try_into().unwrap();
    set(class_hash, 'table', 'key', 0, values.span(), array![251, 251].span());
    let res = get(class_hash, 'table', 'key', 0, values.len(), array![251, 251].span());

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

    let class_hash: starknet::ClassHash = executor::TEST_CLASS_HASH.try_into().unwrap();
    set(class_hash, 'first', 'key', 0, values.span(), array![251, 251].span());
    set(class_hash, 'second', 'key', 0, other.span(), array![251, 251].span());
    let res = get(class_hash, 'first', 'key', 0, values.len(), array![251, 251].span());
    let other_res = get(class_hash, 'second', 'key', 0, other.len(), array![251, 251].span());

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

    let class_hash: starknet::ClassHash = executor::TEST_CLASS_HASH.try_into().unwrap();
    set(class_hash, 'table', 'key', 0, values.span(), array![251, 251].span());
    set(class_hash, 'table', 'other', 0, other.span(), array![251, 251].span());
    let res = get(class_hash, 'table', 'key', 0, values.len(), array![251, 251].span());
    let other_res = get(class_hash, 'table', 'other', 0, other.len(), array![251, 251].span());

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

    let class_hash: starknet::ClassHash = executor::TEST_CLASS_HASH.try_into().unwrap();
    set(class_hash, 'table', 'key', 1, values.span(), array![251, 251, 251, 251, 251].span());
    let first_res = get(class_hash, 'table', 'key', 1, 3, array![251, 251, 251].span());
    let second_res = get(class_hash, 'table', 'key', 3, 5, array![251, 251, 251, 251, 251].span());
    let third_res = get(
        class_hash, 'table', 'key', 5, 7, array![251, 251, 251, 251, 251, 251, 251].span()
    );

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

    let class_hash: starknet::ClassHash = executor::TEST_CLASS_HASH.try_into().unwrap();
    set(class_hash, 'table', 'key', 0, values.span(), array![251].span());

    let before = get(class_hash, 'table', 'key', 0, values.len(), array![251].span());
    assert(*before.at(0) == *values.at(0), 'Values different at index 0!');

    del(class_hash, 'table', 'key');
    let after = get(class_hash, 'table', 'key', 0, 0, array![].span());
    assert(after.len() == 0, 'Non empty after deletion!');
}

#[test]
#[available_gas(10000000)]
fn test_database_all() {
    let mut even = ArrayTrait::new();
    even.append(0x2);
    even.append(0x4);

    let mut odd = ArrayTrait::new();
    even.append(0x1);
    even.append(0x3);

    let class_hash: starknet::ClassHash = executor::TEST_CLASS_HASH.try_into().unwrap();
    set(class_hash, 'table', 'even', 0, even.span(), array![251, 251].span());
    set(class_hash, 'table', 'odd', 0, odd.span(), array![251, 251].span());

    let base = starknet::storage_base_address_from_felt252('table');
    let (keys, values) = all(class_hash, 'table', 0, 2);
}

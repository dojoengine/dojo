use core::traits::Into;
use core::result::ResultTrait;
use array::ArrayTrait;
use option::OptionTrait;
use serde::Serde;
use array::SpanTrait;
use traits::TryInto;

use starknet::syscalls::deploy_syscall;
use starknet::class_hash::Felt252TryIntoClassHash;
use dojo::executor::{executor, IExecutorDispatcher, IExecutorDispatcherTrait};
use dojo::world::{Context, IWorldDispatcher};

use dojo::database::{get, set};



#[test]
#[available_gas(1000000)]
fn test_database_basic() {
    let mut values = ArrayTrait::new();
    values.append('database_test');
    values.append('42');

    let class_hash: starknet::ClassHash = executor::TEST_CLASS_HASH.try_into().unwrap();
    set(class_hash, 'table', 'key', 0, values.span());
    let res = get(class_hash, 'table', 'key', 0, values.len());

    assert(res.at(0) == values.at(0), 'Value at 0 not equal!');
    assert(res.at(1) == values.at(1), 'Value at 0 not equal!');
    assert(res.len() == values.len(), 'Lengths not equal');
}

#[test]
#[available_gas(1000000)]
fn test_database_different_tables() {
    let mut values = ArrayTrait::new();
    values.append(0x1);
    values.append(0x2);

    let mut other = ArrayTrait::new();
    other.append(0x3);
    other.append(0x4);
    
    let class_hash: starknet::ClassHash = executor::TEST_CLASS_HASH.try_into().unwrap();
    set(class_hash, 'first', 'key', 0, values.span());
    set(class_hash, 'second', 'key', 0, other.span());
    let res = get(class_hash, 'first', 'key', 0, values.len());
    let other_res = get(class_hash, 'second', 'key', 0, other.len());

    assert(res.len() == values.len(), 'Lengths not equal');
    assert(res.at(0) == values.at(0), 'Values different at `first`!');
    assert(other_res.at(0) == other_res.at(0), 'Values different at `second`!');
    assert(other_res.at(0) != res.at(0), 'Values the same for different!');
}

#[test]
#[available_gas(1000000)]
fn test_database_different_keys() {
    let mut values = ArrayTrait::new();
    values.append(0x1);
    values.append(0x2);

    let mut other = ArrayTrait::new();
    other.append(0x3);
    other.append(0x4);
    
    let class_hash: starknet::ClassHash = executor::TEST_CLASS_HASH.try_into().unwrap();
    set(class_hash, 'table', 'key', 0, values.span());
    set(class_hash, 'table', 'other', 0, other.span());
    let res = get(class_hash, 'table', 'key', 0, values.len());
    let other_res = get(class_hash, 'table', 'other', 0, other.len());

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
    set(class_hash, 'table', 'key', 1, values.span());
    let first_res = get(class_hash, 'table', 'key', 1, 3);
    let second_res = get(class_hash, 'table', 'key', 3, 5);
    let third_res = get(class_hash, 'table', 'key', 5, 6);

    assert(first_res.at(0) == values.at(0), 'Values different at index 0!');
    assert(first_res.at(1) == values.at(1), 'Values different at index 1!');
    assert(second_res.at(0) == values.at(2), 'Values different at index 2!');
    assert(second_res.at(1) == values.at(3), 'Values different at index 3!');
    assert(third_res.at(0) == values.at(4), 'Values different at index 4!');
}

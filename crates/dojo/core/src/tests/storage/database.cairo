use core::array::{ArrayTrait, SpanTrait};
use core::option::OptionTrait;
use core::result::ResultTrait;
use core::serde::Serde;
use core::traits::{Into, TryInto};

use starknet::class_hash::ClassHash;
use starknet::syscalls::deploy_syscall;

use dojo::storage::database::{get, set, MAX_ARRAY_LENGTH};
use dojo::utils::test::assert_array;
use dojo::world::{IWorldDispatcher};

#[test]
#[available_gas(1000000)]
fn test_database_basic() {
    let mut values = ArrayTrait::new();
    values.append('database_test');
    values.append('42');

    set('table', 'key', values.span(), 0, [251, 251].span());
    let res = get('table', 'key', [251, 251].span());

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

    set('first', 'key', values.span(), 0, [251, 251].span());
    set('second', 'key', other.span(), 0, [251, 251].span());
    let res = get('first', 'key', [251, 251].span());
    let other_res = get('second', 'key', [251, 251].span());

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

    set('table', 'key', values.span(), 0, [251, 251].span());
    set('table', 'other', other.span(), 0, [251, 251].span());
    let res = get('table', 'key', [251, 251].span());
    let other_res = get('table', 'other', [251, 251].span());

    assert(res.len() == values.len(), 'Lengths not equal');
    assert(res.at(0) == values.at(0), 'Values different at `key`!');
    assert(other_res.at(0) == other_res.at(0), 'Values different at `other`!');
    assert(other_res.at(0) != res.at(0), 'Values the same for different!');
}

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
use dojo::database::{get, set, set_with_index, del, scan};
use dojo::database::index::WhereCondition;

#[test]
#[available_gas(2000000)]
fn bench_database() {
    let mut values = ArrayTrait::new();
    values.append('database_test');
    values.append('42');

    set('table', 'key', 0, values.span(), array![251, 251].span());
    set('table', 'key', 0, values.span(), array![251, 251].span());
    set('table', 'key', 0, values.span(), array![251, 251].span());
    let res = get('table', 'key', 0, values.len(), array![251, 251].span());

    assert(res.at(0) == values.at(0), 'Value at 0 not equal!');
    assert(res.at(1) == values.at(1), 'Value at 0 not equal!');
    assert(res.len() == values.len(), 'Lengths not equal');
}
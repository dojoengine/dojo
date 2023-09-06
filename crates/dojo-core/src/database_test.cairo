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
fn test_database() {
    let mut values = ArrayTrait::new();
    values.append('database_test');
    values.append('42');
    
    let class_hash: starknet::ClassHash = executor::TEST_CLASS_HASH.try_into().unwrap();
    set(class_hash, 0, 0, 0, values.span());
    let res = get(class_hash, 0, 0, 0, 0);

    assert(res.len() == 0x0, 'lengths not equal');
}


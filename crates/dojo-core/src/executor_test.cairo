use core::traits::Into;
use core::result::ResultTrait;
use array::ArrayTrait;
use option::OptionTrait;
use serde::Serde;
use traits::TryInto;

use starknet::syscalls::deploy_syscall;
use starknet::class_hash::Felt252TryIntoClassHash;
use dojo::executor::{executor, IExecutorDispatcher, IExecutorDispatcherTrait};
use dojo::world::{IWorldDispatcher};

#[derive(Model, Copy, Drop, Serde)]
struct Foo {
    #[key]
    id: felt252,
    a: felt252,
    b: u128,
}

#[starknet::contract]
mod bar {
    use super::{Foo};

    #[storage]
    struct Storage {}

    #[external(v0)]
    fn dojo_resource(self: @ContractState) -> felt252 {
        'bar'
    }

    #[external(v0)]
    fn execute(self: @ContractState, foo: Foo) -> Foo {
        foo
    }
}

const DOJO_RESOURCE_ENTRYPOINT: felt252 =
    0x0099a4d0ed2dfce68f26fd2ccd22fb86b8215dc58f28638a38220347735906cd;


#[test]
#[available_gas(40000000)]
fn test_executor() {
    let constructor_calldata = array::ArrayTrait::new();
    let (executor_address, _) = deploy_syscall(
        executor::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    )
        .unwrap();

    let executor = IExecutorDispatcher { contract_address: executor_address };

    starknet::testing::set_contract_address(starknet::contract_address_const::<0x1337>());

    let res = *executor
        .call(
            bar::TEST_CLASS_HASH.try_into().unwrap(), DOJO_RESOURCE_ENTRYPOINT, array![].span()
        )[0];

    assert(res == 'bar', 'executor call incorrect')
}

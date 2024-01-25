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

    #[abi(embed_v0)]
    #[generate_trait]
    impl IbarImpl of IBar {
        fn dojo_resource(self: @ContractState) -> felt252 {
            'bar'
        }

        fn execute(self: @ContractState, foo: Foo) -> Foo {
            foo
        }
    }
}

const DOJO_RESOURCE_ENTRYPOINT: felt252 =
    0x038f2d91dabc7079b6f336cc00f874d17cbb7463674c7d3edfd04668fbdb6f6a;


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

use core::traits::Into;
use core::result::ResultTrait;
use array::ArrayTrait;
use option::OptionTrait;
use serde::Serde;
use traits::TryInto;

use starknet::syscalls::deploy_syscall;
use starknet::class_hash::Felt252TryIntoClassHash;
use dojo::executor::{executor, IExecutorDispatcher, IExecutorDispatcherTrait};
use dojo::world::{Context, IWorldDispatcher};

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Foo {
    #[key]
    id: felt252,
    a: felt252,
    b: u128,
}

#[system]
mod Bar {
    use super::Foo;

    fn execute(foo: Foo) -> Foo {
        foo
    }
}

#[test]
#[available_gas(40000000)]
fn test_executor() {
    let constructor_calldata = array::ArrayTrait::new();
    let (executor_address, _) = deploy_syscall(
        executor::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    )
        .unwrap();

    let executor = IExecutorDispatcher { contract_address: executor_address };

    let mut system_calldata = ArrayTrait::new();
    system_calldata.append(1);
    system_calldata.append(42);
    system_calldata.append(53);

    let ctx = Context {
        world: IWorldDispatcher {
            contract_address: starknet::contract_address_const::<0x1337>()
        },
        origin: starknet::contract_address_const::<0x1337>(),
        system: 'Bar',
        system_class_hash: Bar::TEST_CLASS_HASH.try_into().unwrap(),
    };

    ctx.serialize(ref system_calldata);

    starknet::testing::set_contract_address(ctx.world.contract_address);

    let res = executor.execute(ctx.system_class_hash, system_calldata.span());
}


#[test]
#[available_gas(40000000)]
#[should_panic]
fn test_executor_bad_caller() {
    let constructor_calldata = array::ArrayTrait::new();
    let (executor_address, _) = deploy_syscall(
        executor::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    )
        .unwrap();

    let executor = IExecutorDispatcher { contract_address: executor_address };

    let mut system_calldata = ArrayTrait::new();
    system_calldata.append(1);
    system_calldata.append(42);
    system_calldata.append(53);

    let ctx = Context {
        world: IWorldDispatcher {
            contract_address: starknet::contract_address_const::<0x1337>()
        },
        origin: starknet::contract_address_const::<0x1337>(),
        system: 'Bar',
        system_class_hash: Bar::TEST_CLASS_HASH.try_into().unwrap(),
    };

    ctx.serialize(ref system_calldata);

    let res = executor.execute(ctx.system_class_hash, system_calldata.span());
}

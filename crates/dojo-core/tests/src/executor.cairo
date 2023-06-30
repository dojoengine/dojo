use core::traits::Into;
use core::result::ResultTrait;
use array::ArrayTrait;
use option::OptionTrait;
use traits::TryInto;

use starknet::syscalls::deploy_syscall;
use starknet::class_hash::Felt252TryIntoClassHash;
use dojo::interfaces::IExecutorDispatcher;
use dojo::interfaces::IExecutorDispatcherTrait;
use dojo::interfaces::IWorldDispatcher;
use dojo::executor::Executor;
use dojo::world::Context;

#[derive(Component, Copy, Drop, Serde)]
struct Foo {
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
        Executor::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    )
        .unwrap();

    let executor = IExecutorDispatcher { contract_address: executor_address };

    let mut system_calldata = ArrayTrait::new();
    system_calldata.append(42);
    system_calldata.append(53);
    let res = executor
        .execute(
            Context {
                world: IWorldDispatcher {
                    contract_address: starknet::contract_address_const::<0x1337>()
                },
                origin: starknet::contract_address_const::<0x1337>(),
                system: 'Bar',
                system_class_hash: Bar::TEST_CLASS_HASH.try_into().unwrap(),
            },
            system_calldata.span()
        );
}

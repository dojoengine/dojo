use core::traits::Into;
use core::result::ResultTrait;
use array::ArrayTrait;
use option::OptionTrait;
use traits::TryInto;

use starknet::syscalls::deploy_syscall;
use starknet::class_hash::Felt252TryIntoClassHash;
use dojo_core::interfaces::IExecutorDispatcher;
use dojo_core::interfaces::IExecutorDispatcherTrait;
use dojo_core::executor::Executor;

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
#[available_gas(30000000)]
fn test_executor() {
    let constructor_calldata = array::ArrayTrait::new();
    let (executor_address, _) = deploy_syscall(
        Executor::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    ).unwrap();

    let executor = IExecutorDispatcher { contract_address: executor_address };

    let mut system_calldata = ArrayTrait::new();
    system_calldata.append(42);
    system_calldata.append(53);
    let res = executor.execute(
        BarSystem::TEST_CLASS_HASH.try_into().unwrap(), system_calldata.span()
    );
}

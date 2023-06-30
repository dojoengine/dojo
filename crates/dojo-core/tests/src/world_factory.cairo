use core::traits::Into;
use core::result::ResultTrait;
use array::{ArrayTrait, SpanTrait};
use clone::Clone;
use option::OptionTrait;
use traits::TryInto;
use serde::Serde;
use debug::PrintTrait;
use starknet::syscalls::deploy_syscall;
use starknet::get_caller_address;
use starknet::class_hash::ClassHash;
use starknet::class_hash::Felt252TryIntoClassHash;

use dojo::executor::executor;
use dojo::world_factory::{IWorldFactoryDispatcher, IWorldFactoryDispatcherTrait, world_factory};
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait, world};


#[derive(Component, Copy, Drop, Serde)]
struct Foo {
    a: felt252,
    b: u128,
}

#[system]
mod bar {
    use super::Foo;

    fn execute(foo: Foo) -> Foo {
        foo
    }
}

#[test]
#[available_gas(40000000)]
fn test_constructor() {
    let mut calldata: Array<felt252> = array::ArrayTrait::new();
    calldata.append(starknet::class_hash_const::<0x420>().into());
    calldata.append(starknet::contract_address_const::<0x69>().into());

    let (factory_address, _) = deploy_syscall(
        world_factory::TEST_CLASS_HASH.try_into().unwrap(), 0, calldata.span(), false
    )
        .unwrap();

    let factory = IWorldFactoryDispatcher { contract_address: factory_address };

    assert(factory.world() == starknet::class_hash_const::<0x420>(), 'wrong world class hash');
    assert(
        factory.executor() == starknet::contract_address_const::<0x69>(), 'wrong executor contract'
    );
}

#[test]
#[available_gas(90000000)]
fn test_spawn_world() {
    // Deploy Executor
    let constructor_calldata = array::ArrayTrait::new();
    let (executor_address, _) = deploy_syscall(
        executor::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    )
        .unwrap();

    // WorldFactory constructor
    let mut calldata: Array<felt252> = array::ArrayTrait::new();
    calldata.append(world::TEST_CLASS_HASH);
    calldata.append(executor_address.into());

    let (factory_address, _) = deploy_syscall(
        world_factory::TEST_CLASS_HASH.try_into().unwrap(), 0, calldata.span(), false
    )
        .unwrap();

    let factory = IWorldFactoryDispatcher { contract_address: factory_address };

    assert(factory.executor() == executor_address, 'wrong executor address');
    assert(factory.world() == world::TEST_CLASS_HASH.try_into().unwrap(), 'wrong world class hash');

    // Prepare components and systems
    let mut systems: Array<ClassHash> = array::ArrayTrait::new();
    systems.append(bar::TEST_CLASS_HASH.try_into().unwrap());

    let mut components: Array<ClassHash> = array::ArrayTrait::new();
    components.append(foo::TEST_CLASS_HASH.try_into().unwrap());

    // Spawn World from WorldFactory
    let world_address = factory.spawn(components, systems);
    let world = IWorldDispatcher { contract_address: world_address };

    // Check Foo component and Bar system are registered
    let foo_hash = world.component('Foo'.into());
    assert(foo_hash == foo::TEST_CLASS_HASH.try_into().unwrap(), 'component not registered');

    let bar_hash = world.system('bar'.into());
    assert(bar_hash == bar::TEST_CLASS_HASH.try_into().unwrap(), 'system not registered');
}

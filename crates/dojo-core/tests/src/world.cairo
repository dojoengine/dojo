use array::ArrayTrait;
use core::result::ResultTrait;
use traits::Into;
use traits::TryInto;
use option::OptionTrait;
use starknet::class_hash::Felt252TryIntoClassHash;
use starknet::syscalls::deploy_syscall;

use dojo_core::integer::u250;
use dojo_core::integer::U32IntoU250;
use dojo_core::storage::query::QueryTrait;
use dojo_core::interfaces::IWorldDispatcher;
use dojo_core::interfaces::IWorldDispatcherTrait;
use dojo_core::executor::Executor;
use dojo_core::world::World;

#[derive(Component, Copy, Drop, Serde)]
struct Foo {
    a: felt252,
    b: u128,
}

#[test]
#[available_gas(2000000)]
fn test_component() {
    let name = 'Foo'.into();
    World::register_component(FooComponent::TEST_CLASS_HASH.try_into().unwrap());
    let mut data = ArrayTrait::<felt252>::new();
    data.append(1337);
    let id = World::uuid();
    World::set_entity(name, QueryTrait::new_from_id(id.into()), 0_u8, data.span());
    let stored = World::entity(name, QueryTrait::new_from_id(id.into()), 0_u8, 1_usize);
    assert(*stored.snapshot.at(0_usize) == 1337, 'data not stored');
}

#[system]
mod Bar {
    use super::Foo;

    fn execute(foo: Foo) -> Foo {
        foo
    }
}

#[test]
#[available_gas(2000000)]
fn test_system() {
    let executor_constructor_calldata = array::ArrayTrait::<felt252>::new();
    let (executor_address, _) = deploy_syscall(
        Executor::TEST_CLASS_HASH.try_into().unwrap(), 0, executor_constructor_calldata.span(), false
    ).unwrap();

    let mut constructor_calldata = array::ArrayTrait::<felt252>::new();
    constructor_calldata.append('World');
    constructor_calldata.append(executor_address.into());
    let (world_address, _) = deploy_syscall(
        World::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    ).unwrap();
    let world = IWorldDispatcher { contract_address: world_address };

    world.register_system(BarSystem::TEST_CLASS_HASH.try_into().unwrap());
    let mut data = ArrayTrait::<felt252>::new();
    data.append(1337);
    data.append(1337);
    let id = world.uuid();
    world.execute('Bar'.into(), data.span());
}

#[test]
#[available_gas(2000000)]
fn test_constructor() {
    starknet::testing::set_caller_address(starknet::contract_address_const::<0x420>());
    World::constructor(
        'World'.into(),
        starknet::contract_address_const::<0x1337>(),
    );
}

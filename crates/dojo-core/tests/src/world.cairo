use array::ArrayTrait;
use array::SpanTrait;
use clone::Clone;
use core::result::ResultTrait;
use traits::Into;
use traits::TryInto;
use option::OptionTrait;
use starknet::class_hash::Felt252TryIntoClassHash;
use starknet::contract_address_const;
use starknet::ContractAddress;
use starknet::get_caller_address;
use starknet::syscalls::deploy_syscall;

use dojo::executor::executor;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait, library_call, world};

// Components and Systems

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Foo {
    #[key]
    caller: ContractAddress,
    a: felt252,
    b: u128,
}

#[derive(Component, Copy, Drop, Serde, SerdeLen)]
struct Fizz {
    #[key]
    caller: ContractAddress,
    a: felt252
}

#[system]
mod bar {
    use super::Foo;
    use traits::Into;
    use starknet::get_caller_address;
    use dojo::world::Context;

    fn execute(ctx: Context, a: felt252, b: u128) {
        set !(ctx.world, Foo { caller: ctx.origin, a, b });
    }
}

#[system]
mod Buzz {
    use super::{Foo, Fizz};
    use traits::Into;
    use starknet::get_caller_address;
    use dojo::world::Context;

    fn execute(ctx: Context, a: felt252, b: u128) {
        set !(ctx.world, (Foo { caller: ctx.origin, a, b }));
    // let fizz = get !(ctx.world, ctx.origin, Fizz);
    }
}

// Tests

fn deploy_world() -> IWorldDispatcher {
    let mut calldata: Array<felt252> = array::ArrayTrait::new();
    calldata.append(starknet::contract_address_const::<0x0>().into());
    let (world_address, _) = deploy_syscall(
        world::TEST_CLASS_HASH.try_into().unwrap(), 0, calldata.span(), false
    )
        .unwrap();

    IWorldDispatcher { contract_address: world_address }
}

#[test]
#[available_gas(2000000)]
fn test_component() {
    let name = 'Foo';
    let world = deploy_world();

    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());
    let mut keys = ArrayTrait::new();
    keys.append(420);
    let mut data = ArrayTrait::new();
    data.append(1337);

    world.set_entity(name, keys.span(), 0, data.span());
    let stored = world.entity(name, keys.span(), 0, dojo::SerdeLen::<Foo>::len());
    assert(*stored.snapshot.at(0) == 1337, 'data not stored');
}

#[test]
#[available_gas(6000000)]
fn test_system() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());
    let mut data = ArrayTrait::new();
    data.append(1337);
    data.append(1337);
    let id = world.uuid();
    world.execute('bar'.into(), data.span());
}

#[test]
#[available_gas(6000000)]
fn test_emit() {
    let world = deploy_world();

    let mut keys = ArrayTrait::new();
    keys.append('MyEvent');
    let mut values = ArrayTrait::new();
    values.append(1);
    values.append(2);
    world.emit(keys.span(), values.span());
}

#[test]
#[available_gas(9000000)]
fn test_set_entity_admin() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);

    let mut keys = array::ArrayTrait::new();
    keys.append(alice.into());

    let mut data = ArrayTrait::new();
    data.append(420);
    data.append(1337);
    world.execute('bar', data.span());
    let foo = world.entity('Foo', keys.span(), 0, dojo::SerdeLen::<Foo>::len());
    assert(*foo[0] == 420, 'data not stored');
    assert(*foo[1] == 1337, 'data not stored');
}

#[test]
#[available_gas(8000000)]
#[should_panic]
fn test_set_entity_unauthorized() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());

    let caller = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_account_contract_address(caller);

    // Call bar system, should panic as it's not authorized
    let mut data = ArrayTrait::new();
    data.append(420);
    data.append(1337);
    world.execute('bar'.into(), data.span());
}

#[test]
#[available_gas(8000000)]
#[should_panic]
fn test_set_entity_directly() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());

    set !(world, Foo { caller: starknet::contract_address_const::<0x1337>(), a: 420, b: 1337 });
}

// Utils
fn spawn_empty_world() -> IWorldDispatcher {
    // Deploy executor contract
    let executor_constructor_calldata = array::ArrayTrait::new();
    let (executor_address, _) = deploy_syscall(
        executor::TEST_CLASS_HASH.try_into().unwrap(),
        0,
        executor_constructor_calldata.span(),
        false
    )
        .unwrap();

    // Deploy world contract
    let mut constructor_calldata = array::ArrayTrait::new();
    constructor_calldata.append(executor_address.into());
    let (world_address, _) = deploy_syscall(
        world::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    )
        .unwrap();
    let world = IWorldDispatcher { contract_address: world_address };

    world
}

#[test]
#[available_gas(6000000)]
fn test_library_call_system() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(library_call::TEST_CLASS_HASH.try_into().unwrap());
    let mut calldata = ArrayTrait::new();
    calldata.append(foo::TEST_CLASS_HASH);
    // 'name' entrypoint
    calldata.append(0x0361458367e696363fbcc70777d07ebbd2394e89fd0adcaf147faccd1d294d60);
    calldata.append(0);
    world.execute('library_call'.into(), calldata.span());
}

#[test]
#[available_gas(6000000)]
fn test_owner() {
    let world = spawn_empty_world();

    let alice = starknet::contract_address_const::<0x1337>();
    let bob = starknet::contract_address_const::<0x1338>();

    assert(!world.is_owner(alice, 0), 'should not be owner');
    assert(!world.is_owner(bob, 42), 'should not be owner');

    world.grant_owner(alice, 0);
    assert(world.is_owner(alice, 0), 'should be owner');

    world.grant_owner(bob, 42);
    assert(world.is_owner(bob, 42), 'should be owner');

    world.revoke_owner(alice, 0);
    assert(!world.is_owner(alice, 0), 'should not be owner');

    world.revoke_owner(bob, 42);
    assert(!world.is_owner(bob, 42), 'should not be owner');
}

#[test]
#[available_gas(6000000)]
#[should_panic]
fn test_set_owner_fails_for_non_owner() {
    let world = spawn_empty_world();

    let alice = starknet::contract_address_const::<0x1337>();

    starknet::testing::set_contract_address(alice);

    world.revoke_owner(alice, 0);
    assert(!world.is_owner(alice, 0), 'should not be owner');

    world.grant_owner(alice, 0);
}

#[test]
#[available_gas(6000000)]
fn test_writer() {
    let world = spawn_empty_world();

    assert(!world.is_writer(42, 69), 'should not be writer');

    world.grant_writer(42, 69);
    assert(world.is_writer(42, 69), 'should be writer');

    world.revoke_writer(42, 69);
    assert(!world.is_writer(42, 69), 'should not be writer');
}

#[test]
#[available_gas(6000000)]
#[should_panic]
fn test_set_writer_fails_for_non_owner() {
    let world = spawn_empty_world();

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);
    assert(!world.is_owner(alice, 0), 'should not be owner');

    world.grant_writer(42, 69);
}

#[system]
mod origin {
    use dojo::world::Context;

    fn execute(ctx: Context) {
        assert(ctx.origin == starknet::contract_address_const::<0x1337>(), 'should be equal');
    }
}

#[system]
mod origin_wrapper {
    use traits::Into;
    use array::ArrayTrait;
    use dojo::world::Context;

    fn execute(ctx: Context) {
        let data = ArrayTrait::new();
        assert(ctx.origin == starknet::contract_address_const::<0x1337>(), 'should be equal');
        ctx.world.execute('origin'.into(), data.span());
        assert(ctx.origin == starknet::contract_address_const::<0x1337>(), 'should be equal');
    }
}

#[test]
#[available_gas(6000000)]
fn test_execute_origin() {
    // Spawn empty world
    let world = spawn_empty_world();

    world.register_system(origin::TEST_CLASS_HASH.try_into().unwrap());
    world.register_system(origin_wrapper::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());
    let data = ArrayTrait::new();

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);
    assert(world.origin() == starknet::contract_address_const::<0x0>(), 'should be equal');
    world.execute('origin_wrapper'.into(), data.span());
    assert(world.origin() == starknet::contract_address_const::<0x0>(), 'should be equal');
}

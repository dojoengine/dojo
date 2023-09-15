use array::{ArrayTrait, SpanTrait};
use clone::Clone;
use core::result::ResultTrait;
use traits::{Into, TryInto};
use option::OptionTrait;
use starknet::class_hash::Felt252TryIntoClassHash;
use starknet::{contract_address_const, ContractAddress, get_caller_address};
use starknet::syscalls::deploy_syscall;

use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait, library_call, world};
use dojo::test_utils::spawn_test_world;

// Components and Systems

#[derive(Component, Copy, Drop, Serde)]
struct Foo {
    #[key]
    caller: ContractAddress,
    a: felt252,
    b: u128,
}

#[derive(Component, Copy, Drop, Serde)]
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
        set!(ctx.world, Foo { caller: ctx.origin, a, b });
    }
}

// Tests

#[test]
#[available_gas(2000000)]
fn test_component() {
    let name = 'Foo';
    let world = deploy_world();

    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[available_gas(6000000)]
fn test_system() {
    // Spawn empty world
    let world = deploy_world();

    world.register_system(bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());
    let mut data = ArrayTrait::new();
    data.append(1337);
    data.append(1337);
    let id = world.uuid();
    world.execute('bar', data);

    let mut keys = ArrayTrait::new();
    keys.append(0);

    let stored = world.entity('Foo', keys.span(), 0, dojo::StorageSize::<Foo>::unpacked_size());
    assert(*stored.snapshot.at(0) == 1337, 'data not stored');
}

#[test]
#[available_gas(6000000)]
fn test_class_hash_getters() {
    let world = deploy_world();

    world.register_system(bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = world.component('Foo');
    assert(foo == foo::TEST_CLASS_HASH.try_into().unwrap(), 'foo does not exists');
    let bar = world.system('bar');
    // Systems are not class hashes anymore
    // assert(bar == bar::TEST_CLASS_HASH.try_into().unwrap(), 'bar does not exists');
    assert(bar.is_non_zero(), 'bar does not exists');
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
    world.emit(keys, values.span());
}

#[test]
#[available_gas(9000000)]
fn test_set_entity_admin() {
    // Spawn empty world
    let world = deploy_world();

    world.register_system(bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);

    let mut keys = array::ArrayTrait::new();
    keys.append(alice.into());

    let mut data = ArrayTrait::new();
    data.append(420);
    data.append(1337);
    world.execute('bar', data);
    let foo = world.entity('Foo', keys.span(), 0, dojo::StorageSize::<Foo>::unpacked_size());
    assert(*foo[0] == 420, 'data not stored');
    assert(*foo[1] == 1337, 'data not stored');
}

#[test]
#[available_gas(8000000)]
#[should_panic]
fn test_set_entity_unauthorized() {
    // Spawn empty world
    let world = deploy_world();

    world.register_system(bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());

    let caller = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_account_contract_address(caller);

    // Call bar system, should panic as it's not authorized
    let mut data = ArrayTrait::new();
    data.append(420);
    data.append(1337);
    world.execute('bar', data);
}

#[test]
#[available_gas(8000000)]
#[should_panic]
fn test_set_entity_invalid_data() {
    // Spawn empty world
    let world = deploy_world();

    world.register_system(bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());

    let caller = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_account_contract_address(caller);

    // Call bar system, should panic as data is invalid
    let mut data = ArrayTrait::new();
    data.append(420);
    world.execute('bar', data);
}

#[test]
#[available_gas(8000000)]
#[should_panic]
fn test_set_entity_excess_data() {
    // Spawn empty world
    let world = deploy_world();

    world.register_system(bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());

    let caller = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_account_contract_address(caller);

    // Call bar system, should panic as it's not authorized
    let mut data = ArrayTrait::new();
    data.append(420);
    data.append(420);
    data.append(420);
    world.execute('bar', data);
}

#[test]
#[available_gas(8000000)]
#[should_panic]
fn test_set_entity_directly() {
    // Spawn empty world
    let world = deploy_world();

    world.register_system(bar::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());

    set!(world, Foo { caller: starknet::contract_address_const::<0x1337>(), a: 420, b: 1337 });
}

// Utils
fn deploy_world() -> IWorldDispatcher {
    spawn_test_world(array![], array![])
}

#[test]
#[available_gas(6000000)]
fn test_library_call_system() {
    // Spawn empty world
    let world = deploy_world();

    world.register_system(library_call::TEST_CLASS_HASH.try_into().unwrap());
    let mut calldata = ArrayTrait::new();
    calldata.append(foo::TEST_CLASS_HASH);
    // 'name' entrypoint
    calldata.append(0x0361458367e696363fbcc70777d07ebbd2394e89fd0adcaf147faccd1d294d60);
    calldata.append(0);
    world.execute('library_call', calldata);
}

#[test]
#[available_gas(6000000)]
fn test_owner() {
    let world = deploy_world();

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
    let world = deploy_world();

    let alice = starknet::contract_address_const::<0x1337>();

    starknet::testing::set_contract_address(alice);

    world.revoke_owner(alice, 0);
    assert(!world.is_owner(alice, 0), 'should not be owner');

    world.grant_owner(alice, 0);
}

#[test]
#[available_gas(6000000)]
fn test_writer() {
    let world = deploy_world();

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
    let world = deploy_world();

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

#[test]
#[available_gas(6000000)]
fn test_execute_origin() {
    // Spawn empty world
    let world = deploy_world();

    world.register_system(origin::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());
    let data = ArrayTrait::new();

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);
    world.execute('origin', data);
}

#[test]
#[available_gas(6000000)]
#[should_panic]
fn test_execute_origin_failing() {
    // Spawn empty world
    let world = deploy_world();

    world.register_system(origin::TEST_CLASS_HASH.try_into().unwrap());
    world.register_component(foo::TEST_CLASS_HASH.try_into().unwrap());
    let data = ArrayTrait::new();

    let eve = starknet::contract_address_const::<0x1338>();
    world.execute('origin', data);
}

#[test]
#[available_gas(6000000)]
fn test_execute_multiple_worlds() {
    // Deploy world contract
    let world = spawn_test_world(array![foo::TEST_CLASS_HASH], array![bar::TEST_CLASS_HASH],);

    // Deploy another world contract
    let another_world = spawn_test_world(
        array![foo::TEST_CLASS_HASH], array![bar::TEST_CLASS_HASH],
    );

    let mut data = ArrayTrait::new();
    data.append(1337);
    data.append(1337);
    let mut another_data = ArrayTrait::new();
    another_data.append(7331);
    another_data.append(7331);
    let mut keys = ArrayTrait::new();
    keys.append(0);

    world.execute('bar', data);
    another_world.execute('bar', another_data);

    let stored = world.entity('Foo', keys.span(), 0, dojo::StorageSize::<Foo>::unpacked_size());
    let another_stored = another_world
        .entity('Foo', keys.span(), 0, dojo::StorageSize::<Foo>::unpacked_size());
    assert(*stored.snapshot.at(0) == 1337, 'data not stored');
    assert(*another_stored.snapshot.at(0) == 7331, 'data not stored');
}

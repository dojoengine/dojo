use array::{ArrayTrait, SpanTrait};
use clone::Clone;
use core::result::ResultTrait;
use traits::{Into, TryInto};
use option::OptionTrait;
use starknet::class_hash::Felt252TryIntoClassHash;
use starknet::{contract_address_const, ContractAddress, ClassHash, get_caller_address};
use starknet::syscalls::deploy_syscall;

use dojo::executor::executor;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait, world};
use dojo::database::schema::SchemaIntrospection;
use dojo::test_utils::{spawn_test_world, deploy_with_world_address};

#[derive(Model, Copy, Drop, Serde)]
struct Foo {
    #[key]
    caller: ContractAddress,
    a: felt252,
    b: u128,
}

#[derive(Model, Copy, Drop, Serde)]
struct Fizz {
    #[key]
    caller: ContractAddress,
    a: felt252
}

#[starknet::interface]
trait Ibar<TContractState> {
    fn set_foo(self: @TContractState, a: felt252, b: u128);
}

#[starknet::contract]
mod bar {
    use super::{Foo, IWorldDispatcher, IWorldDispatcherTrait};
    use traits::Into;
    use starknet::{get_caller_address, ContractAddress};

    #[storage]
    struct Storage {
        world: IWorldDispatcher,
    }
    #[constructor]
    fn constructor(ref self: ContractState, world: ContractAddress) {
        self.world.write(IWorldDispatcher { contract_address: world })
    }

    #[external(v0)]
    impl IbarImpl of super::Ibar<ContractState> {
        fn set_foo(self: @ContractState, a: felt252, b: u128) {
            set!(self.world.read(), Foo { caller: get_caller_address(), a, b });
        }
    }
}

// Tests

#[test]
#[available_gas(2000000)]
fn test_model() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[available_gas(6000000)]
fn test_system() {
    // Spawn empty world
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    // System contract
    let bar_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
    };

    bar_contract.set_foo(1337, 1337);

    let stored: Foo = get!(world, get_caller_address(), Foo);
    assert(stored.a == 1337, 'data not stored');
    assert(stored.b == 1337, 'data not stored');
}

#[test]
#[available_gas(6000000)]
fn test_model_class_hash_getter() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo = world.model('Foo');
    assert(foo == foo::TEST_CLASS_HASH.try_into().unwrap(), 'foo does not exists');
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
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let bar_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
    };

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);

    bar_contract.set_foo(420, 1337);

    let foo: Foo = get!(world, alice, Foo);
    assert(foo.a == 420, 'data not stored');
    assert(foo.b == 1337, 'data not stored');
}

#[test]
#[available_gas(8000000)]
#[should_panic]
fn test_set_entity_unauthorized() {
    // Spawn empty world
    let world = deploy_world();

    let bar_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
    };

    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let caller = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_account_contract_address(caller);

    // Call bar system, should panic as it's not authorized
    bar_contract.set_foo(420, 1337);
}

// This test is probably irrelevant now because we have no systems,
// so all `set_entity` call are from arbitrary contracts.
// Owners can still update via unregistered contracts/call from account
// #[test]
// #[available_gas(8000000)]
// #[should_panic]
// fn test_set_entity_directly() {
//     // Spawn world
//     let world = deploy_world();
//     world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

//     let bar_contract = IbarDispatcher {
//         contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
//     };

//     set!(world, Foo { caller: starknet::contract_address_const::<0x1337>(), a: 420, b: 1337 });
// }

// Utils
fn deploy_world() -> IWorldDispatcher {
    spawn_test_world(array![])
}

#[test]
#[available_gas(60000000)]
fn test_entities() {
    // Deploy world contract
    let world = spawn_test_world(array![foo::TEST_CLASS_HASH],);

    let bar_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
    };

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);
    bar_contract.set_foo(1337, 1337);

    let mut keys = ArrayTrait::new();
    keys.append(0);

    let mut query_keys = ArrayTrait::new().span();
    let keys_layout = array![251].span();
    let (keys, values) = world.entities('Foo', Option::None(()), query_keys, 2, keys_layout);
    assert(keys.len() == 1, 'No keys found for any!');

    // query_keys.append(0x1337);
    // let (keys, values) = world.entities('Foo', 42, query_keys.span(), 2, keys_layout);
    // assert(keys.len() == 1, 'No keys found!');

    // let mut query_keys = ArrayTrait::new();
    // query_keys.append(0x1338);
    // let (keys, values) = world.entities('Foo', 42, query_keys.span(), 2, keys_layout);
    // assert(keys.len() == 0, 'Keys found!');
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

    assert(!world.is_writer(42, 69.try_into().unwrap()), 'should not be writer');

    world.grant_writer(42, 69.try_into().unwrap());
    assert(world.is_writer(42, 69.try_into().unwrap()), 'should be writer');

    world.revoke_writer(42, 69.try_into().unwrap());
    assert(!world.is_writer(42, 69.try_into().unwrap()), 'should not be writer');
}

#[test]
#[available_gas(6000000)]
#[should_panic]
fn test_system_not_writer_fail() {
    let world = spawn_test_world(array![foo::TEST_CLASS_HASH],);

    let bar_address = deploy_with_world_address(bar::TEST_CLASS_HASH, world);
    let bar_contract = IbarDispatcher { contract_address: bar_address };

    // Caller is not owner now
    let caller = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_account_contract_address(caller);

    // Should panic, system not writer
    bar_contract.set_foo(25, 16);
}

#[test]
#[available_gas(6000000)]
fn test_system_writer_access() {
    let world = spawn_test_world(array![foo::TEST_CLASS_HASH],);

    let bar_address = deploy_with_world_address(bar::TEST_CLASS_HASH, world);
    let bar_contract = IbarDispatcher { contract_address: bar_address };

    world.grant_writer('Foo', bar_address);
    assert(world.is_writer('Foo', bar_address), 'should be writer');

    // Caller is not owner now
    let caller = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_account_contract_address(caller);

    // Should not panic, system is writer
    bar_contract.set_foo(25, 16);
}

#[test]
#[available_gas(6000000)]
#[should_panic]
fn test_set_writer_fails_for_non_owner() {
    let world = deploy_world();

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);

    assert(!world.is_owner(alice, 0), 'should not be owner');

    world.grant_writer(42, 69.try_into().unwrap());
}


#[starknet::interface]
trait IOrigin<TContractState> {
    fn assert_origin(self: @TContractState);
}

#[starknet::contract]
mod origin {
    use super::{IWorldDispatcher, ContractAddress};

    #[storage]
    struct Storage {
        world: IWorldDispatcher,
    }

    #[constructor]
    fn constructor(ref self: ContractState, world: ContractAddress) {
        self.world.write(IWorldDispatcher { contract_address: world })
    }

    #[external(v0)]
    impl IOriginImpl of super::IOrigin<ContractState> {
        fn assert_origin(self: @ContractState) {
            assert(
                starknet::get_caller_address() == starknet::contract_address_const::<0x1337>(),
                'should be equal'
            );
        }
    }
}

#[test]
#[available_gas(60000000)]
fn test_execute_multiple_worlds() {
    // Deploy world contract
    let world1 = spawn_test_world(array![foo::TEST_CLASS_HASH],);

    let bar1_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world1)
    };

    // Deploy another world contract
    let world2 = spawn_test_world(array![foo::TEST_CLASS_HASH],);

    let bar2_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world2)
    };

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);

    bar1_contract.set_foo(1337, 1337);
    bar2_contract.set_foo(7331, 7331);

    let mut keys = ArrayTrait::new();
    keys.append(0);

    let data1 = get!(world1, alice, Foo);
    let data2 = get!(world2, alice, Foo);
    assert(data1.a == 1337, 'data1 not stored');
    assert(data2.a == 7331, 'data2 not stored');
}



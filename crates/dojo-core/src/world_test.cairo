use array::{ArrayTrait, SpanTrait};
use clone::Clone;
use core::result::ResultTrait;
use traits::{Into, TryInto};
use option::OptionTrait;
use starknet::class_hash::Felt252TryIntoClassHash;
use starknet::{contract_address_const, ContractAddress, ClassHash, get_caller_address};
use starknet::syscalls::deploy_syscall;

use dojo::benchmarks;
use dojo::executor::executor;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait, world, IUpgradeableWorld, IUpgradeableWorldDispatcher, IUpgradeableWorldDispatcherTrait };
use dojo::database::introspect::Introspect;
use dojo::test_utils::{spawn_test_world, deploy_with_world_address};
use dojo::benchmarks::{Character, end};

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
    fn delete_foo(self: @TContractState);
    fn delete_foo_macro(self: @TContractState, foo: Foo);
    fn set_char(self: @TContractState, a: felt252, b: u32);
}

#[starknet::contract]
mod bar {
    use super::{Foo, IWorldDispatcher, IWorldDispatcherTrait, Introspect};
    use super::benchmarks::{Character, Abilities, Stats, Weapon, Sword};
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

        fn delete_foo(self: @ContractState) {
            let mut layout = array![];
            Introspect::<Foo>::layout(ref layout);
            self
                .world
                .read()
                .delete_entity('Foo', array![get_caller_address().into()].span(), layout.span());
        }

        fn delete_foo_macro(self: @ContractState, foo: Foo) {
            delete!(self.world.read(), Foo { caller: foo.caller, a: foo.a, b: foo.b });
        }

        fn set_char(self: @ContractState, a: felt252, b: u32) {
            set!(
                self.world.read(),
                Character {
                    caller: get_caller_address(),
                    heigth: a,
                    abilities: Abilities {
                        strength: 0x12,
                        dexterity: 0x34,
                        constitution: 0x56,
                        intelligence: 0x78,
                        wisdom: 0x9a,
                        charisma: 0xbc,
                    },
                    stats: Stats {
                        kills: 0x123456789abcdef,
                        deaths: 0x1234,
                        rests: 0x12345678,
                        hits: 0x123456789abcdef,
                        blocks: 0x12345678,
                        walked: 0x123456789abcdef,
                        runned: 0x123456789abcdef,
                        finished: true,
                        romances: 0x1234,
                    },
                    weapon: Weapon::DualWield(
                        (
                            Sword { swordsmith: get_caller_address(), damage: 0x12345678, },
                            Sword { swordsmith: get_caller_address(), damage: 0x12345678, }
                        )
                    ),
                    gold: b,
                }
            );
        }
    }
}

// Tests

fn deploy_world_and_bar() -> (IWorldDispatcher, IbarDispatcher) {
    // Spawn empty world
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    // System contract
    let bar_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
    };

    (world, bar_contract)
}

#[test]
#[available_gas(2000000)]
fn test_model() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[available_gas(6000000)]
fn test_system() {
    let (world, bar_contract) = deploy_world_and_bar();

    bar_contract.set_foo(1337, 1337);

    let stored: Foo = get!(world, get_caller_address(), Foo);
    assert(stored.a == 1337, 'data not stored');
    assert(stored.b == 1337, 'data not stored');
}

#[test]
#[available_gas(6000000)]
fn test_delete() {
    let (world, bar_contract) = deploy_world_and_bar();

    // set model
    bar_contract.set_foo(1337, 1337);
    let stored: Foo = get!(world, get_caller_address(), Foo);
    assert(stored.a == 1337, 'data not stored');
    assert(stored.b == 1337, 'data not stored');

    // delete model
    bar_contract.delete_foo_macro(stored);

    let deleted: Foo = get!(world, get_caller_address(), Foo);
    assert(deleted.a == 0, 'data not deleted');
    assert(deleted.b == 0, 'data not deleted');
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
    let (world, bar_contract) = deploy_world_and_bar();

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
fn test_metadata_uri() {
    // Deploy world contract
    let world = deploy_world();
    world.set_metadata_uri(0, array!['test_uri'].span());
    let uri = world.metadata_uri(0);

    assert(uri.len() == 1, 'Incorrect metadata uri len');
    assert(uri[0] == @'test_uri', 'Incorrect metadata uri');

    world.set_metadata_uri(0, array!['new_uri', 'longer'].span());

    let uri = world.metadata_uri(0);
    assert(uri.len() == 2, 'Incorrect metadata uri len');
    assert(uri[0] == @'new_uri', 'Incorrect metadata uri 1');
    assert(uri[1] == @'longer', 'Incorrect metadata uri 2');
}

#[test]
#[available_gas(60000000)]
#[should_panic]
fn test_set_metadata_uri_reverts_for_not_owner() {
    // Deploy world contract
    let world = deploy_world();

    starknet::testing::set_contract_address(starknet::contract_address_const::<0x1337>());
    world.set_metadata_uri(0, array!['new_uri', 'longer'].span());
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

    let mut query_keys = ArrayTrait::new();
    let layout = array![251].span();
    let (keys, values) = world.entities('Foo', Option::None, query_keys.span(), 2, layout);
    let ids = world.entity_ids('Foo');
    assert(keys.len() == ids.len(), 'result differs in entity_ids');
    assert(keys.len() == 0, 'found value for unindexed');
// query_keys.append(0x1337);
// let (keys, values) = world.entities('Foo', 42, query_keys.span(), 2, layout);
// assert(keys.len() == 1, 'No keys found!');

// let mut query_keys = ArrayTrait::new();
// query_keys.append(0x1338);
// let (keys, values) = world.entities('Foo', 42, query_keys.span(), 2, layout);
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

    let data1 = get!(world1, alice, Foo);
    let data2 = get!(world2, alice, Foo);
    assert(data1.a == 1337, 'data1 not stored');
    assert(data2.a == 7331, 'data2 not stored');
}

#[test]
#[available_gas(60000000)]
fn bench_execute() {
    let world = spawn_test_world(array![foo::TEST_CLASS_HASH],);
    let bar_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
    };

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    bar_contract.set_foo(1337, 1337);
    end(gas, 'foo set call');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let data = get!(world, alice, Foo);
    end(gas, 'foo get macro');

    assert(data.a == 1337, 'data not stored');
}

#[test]
#[available_gas(60000000)]
fn bench_execute_complex() {
    let world = spawn_test_world(array![foo::TEST_CLASS_HASH],);
    let bar_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
    };

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    bar_contract.set_char(1337, 1337);
    end(gas, 'char set call');

    let gas = testing::get_available_gas();
    gas::withdraw_gas().unwrap();
    let data = get!(world, alice, Character);
    end(gas, 'char get macro');

    assert(data.heigth == 1337, 'data not stored');
}


#[starknet::interface]
trait IWorldUpgrade<TContractState> {
    fn hello(self: @TContractState) -> felt252;
}

#[starknet::contract]
mod worldupgrade {
    use super::{IWorldUpgrade, IWorldDispatcher, ContractAddress};

    #[storage]
    struct Storage {
        world: IWorldDispatcher,
    }
    
    #[external(v0)]
    impl IWorldUpgradeImpl of super::IWorldUpgrade<ContractState> {
        fn hello(self: @ContractState) -> felt252{
            'dojo'
        }
    }
}


#[test]
#[available_gas(60000000)]
fn test_upgradeable_world() {
    
    // Deploy world contract
    let world = deploy_world();

    let mut upgradeable_world_dispatcher = IUpgradeableWorldDispatcher {
        contract_address: world.contract_address
    };
    upgradeable_world_dispatcher.upgrade(worldupgrade::TEST_CLASS_HASH.try_into().unwrap());

    let res = (IWorldUpgradeDispatcher {
        contract_address: world.contract_address
    }).hello();

    assert(res == 'dojo', 'should return dojo');
}

#[test]
#[available_gas(60000000)]
#[should_panic(expected:('invalid class_hash', 'ENTRYPOINT_FAILED'))]
fn test_upgradeable_world_with_class_hash_zero() {
    
    // Deploy world contract
    let world = deploy_world();

    starknet::testing::set_contract_address(starknet::contract_address_const::<0x1337>());

    let mut upgradeable_world_dispatcher = IUpgradeableWorldDispatcher {
        contract_address: world.contract_address
    };
    upgradeable_world_dispatcher.upgrade(0.try_into().unwrap());
}

#[test]
#[available_gas(60000000)]
#[should_panic( expected: ('only owner can upgrade', 'ENTRYPOINT_FAILED'))]
fn test_upgradeable_world_from_non_owner() {
    
    // Deploy world contract
    let world = deploy_world();

    let not_owner = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(not_owner);
    starknet::testing::set_account_contract_address(not_owner);

    let mut upgradeable_world_dispatcher = IUpgradeableWorldDispatcher {
        contract_address: world.contract_address
    };
    upgradeable_world_dispatcher.upgrade(worldupgrade::TEST_CLASS_HASH.try_into().unwrap());
}
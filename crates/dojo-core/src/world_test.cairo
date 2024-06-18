use array::{ArrayTrait, SpanTrait};
use clone::Clone;
use core::result::ResultTrait;
use traits::{Into, TryInto};
use option::OptionTrait;
use starknet::class_hash::Felt252TryIntoClassHash;
use starknet::{contract_address_const, ContractAddress, ClassHash, get_caller_address};
use starknet::syscalls::deploy_syscall;

use dojo::benchmarks;
use dojo::config::interface::{IConfigDispatcher, IConfigDispatcherImpl};
use dojo::world::{
    IWorldDispatcher, IWorldDispatcherTrait, world, IUpgradeableWorld, IUpgradeableWorldDispatcher,
    IUpgradeableWorldDispatcherTrait, ResourceMetadata
};
use dojo::database::introspect::{Introspect, Layout, FieldLayout};
use dojo::database::MAX_ARRAY_LENGTH;
use dojo::test_utils::{spawn_test_world, deploy_with_world_address, assert_array};
use dojo::config::component::Config::{
    DifferProgramHashUpdate, MergerProgramHashUpdate, FactsRegistryUpdate
};
use dojo::model::Model;
use dojo::benchmarks::{Character, GasCounterImpl};

#[derive(Introspect, Copy, Drop, Serde)]
enum OneEnum {
    FirstArm: (u8, felt252),
    SecondArm,
}

#[derive(Introspect, Drop, Serde)]
enum AnotherEnum {
    FirstArm: (u8, OneEnum, ByteArray),
    SecondArm: (u8, OneEnum, ByteArray)
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
struct Foo {
    #[key]
    caller: ContractAddress,
    a: felt252,
    b: u128,
}

fn create_foo() -> Span<felt252> {
    array![1, 2].span()
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
struct Fizz {
    #[key]
    caller: ContractAddress,
    a: felt252
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
struct StructSimpleModel {
    #[key]
    caller: ContractAddress,
    a: felt252,
    b: u128,
}

fn create_struct_simple_model() -> Span<felt252> {
    array![1, 2].span()
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
struct StructWithTuple {
    #[key]
    caller: ContractAddress,
    a: (u8, u64)
}

fn create_struct_with_tuple() -> Span<felt252> {
    array![12, 58].span()
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
struct StructWithEnum {
    #[key]
    caller: ContractAddress,
    a: OneEnum,
}

fn create_struct_with_enum_first_variant() -> Span<felt252> {
    array![0, 1, 2].span()
}

fn create_struct_with_enum_second_variant() -> Span<felt252> {
    array![1].span()
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
struct StructSimpleArrayModel {
    #[key]
    caller: ContractAddress,
    a: felt252,
    b: Array<u64>,
    c: u128,
}

impl ArrayU64Copy of core::traits::Copy<Array<u64>>;

fn create_struct_simple_array_model() -> Span<felt252> {
    array![1, 4, 10, 20, 30, 40, 2].span()
}

#[derive(Drop, Serde)]
#[dojo::model]
struct StructByteArrayModel {
    #[key]
    caller: ContractAddress,
    a: felt252,
    b: ByteArray,
}

fn create_struct_byte_array_model() -> Span<felt252> {
    array![1, 3, 'first', 'second', 'third', 'pending', 7].span()
}

#[derive(Introspect, Copy, Drop, Serde)]
struct ModelData {
    x: u256,
    y: u32,
    z: felt252
}

#[derive(Drop, Serde)]
#[dojo::model]
struct StructComplexArrayModel {
    #[key]
    caller: ContractAddress,
    a: felt252,
    b: Array<ModelData>,
    c: AnotherEnum,
}

fn create_struct_complex_array_model() -> Span<felt252> {
    array![
        1, // a
        2, // b (array length)
        1,
        2,
        3,
        4, // item 1
        5,
        6,
        7,
        8, // item 2
        1, // c (AnotherEnum variant)
        1, // u8
        0, // OneEnum variant
        0, // u8
        123, // felt252
        1,
        'first',
        'pending',
        7 // ByteArray
    ]
        .span()
}

#[derive(Drop, Serde)]
#[dojo::model]
struct StructNestedModel {
    #[key]
    caller: ContractAddress,
    x: (u8, u16, (u32, ByteArray, u8), Array<(u8, u16)>),
    y: Array<Array<(u8, (u16, u256))>>
}

fn create_struct_nested_model() -> Span<felt252> {
    array![
        // -- x
        1, // u8
        2, // u16
        3,
        1,
        'first',
        'pending',
        7,
        9, // (u32, ByteArray, u8)
        3,
        1,
        2,
        3,
        4,
        5,
        6, // Array<(u8, u16)> with 3 items
        // -- y
        2, // Array<Array<(u8, (u16, u256))>> with 2 items
        3, // first array item - Array<(u8, (u16, u256))> of 3 items
        1,
        2,
        0,
        3, // first array item - (u8, (u16, u256))
        4,
        5,
        0,
        6, // second array item - (u8, (u16, u256))
        8,
        7,
        9,
        10, // third array item - (u8, (u16, u256))
        1, // second array item - Array<(u8, (u16, u256))> of 1 item
        5,
        4,
        6,
        7 // first array item - (u8, (u16, u256))
    ]
        .span()
}

#[derive(Introspect, Copy, Drop, Serde)]
enum EnumGeneric<T, U> {
    One: T,
    Two: U
}

#[derive(Drop, Serde)]
#[dojo::model]
struct StructWithGeneric {
    #[key]
    caller: ContractAddress,
    x: EnumGeneric<u8, u256>,
}

fn create_struct_generic_first_variant() -> Span<felt252> {
    array![0, 1].span()
}

fn create_struct_generic_second_variant() -> Span<felt252> {
    array![1, 1, 2].span()
}

fn get_key_test() -> Span<felt252> {
    array![0x01234].span()
}

#[starknet::interface]
trait IMetadataOnly<T> {
    fn selector(self: @T) -> felt252;
    fn name(self: @T) -> ByteArray;
}

#[starknet::contract]
mod resource_metadata_malicious {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl InvalidModelName of super::IMetadataOnly<ContractState> {
        fn selector(self: @ContractState) -> felt252 {
            selector!("ResourceMetadata")
        }

        fn name(self: @ContractState) -> ByteArray {
            "invalid_model_name"
        }
    }
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

    #[abi(embed_v0)]
    impl IbarImpl of super::Ibar<ContractState> {
        fn set_foo(self: @ContractState, a: felt252, b: u128) {
            set!(self.world.read(), Foo { caller: get_caller_address(), a, b });
        }

        fn delete_foo(self: @ContractState) {
            self
                .world
                .read()
                .delete_entity(
                    selector!("Foo"),
                    array![get_caller_address().into()].span(),
                    dojo::model::Model::<Foo>::layout()
                );
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
fn test_system() {
    let (world, bar_contract) = deploy_world_and_bar();

    bar_contract.set_foo(1337, 1337);

    let stored: Foo = get!(world, get_caller_address(), Foo);
    assert(stored.a == 1337, 'data not stored');
    assert(stored.b == 1337, 'data not stored');
}

#[test]
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
use core::debug::PrintTrait;

#[test]
#[available_gas(6000000)]
fn test_model_class_hash_getter() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let (foo_class_hash, _) = world.model(selector!("Foo"));
    assert(foo_class_hash == foo::TEST_CLASS_HASH.try_into().unwrap(), 'foo wrong class hash');
}

#[test]
#[ignore]
#[available_gas(6000000)]
fn test_legacy_model_class_hash_getter() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let (foo_class_hash, _) = world.model('Foo');
    assert(foo_class_hash == foo::TEST_CLASS_HASH.try_into().unwrap(), 'foo wrong class hash');
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

// Utils
fn deploy_world() -> IWorldDispatcher {
    spawn_test_world(array![])
}

#[test]
#[available_gas(60000000)]
fn test_set_metadata_world() {
    let world = deploy_world();

    let metadata = ResourceMetadata {
        resource_id: 0, metadata_uri: format!("ipfs:world_with_a_long_uri_that")
    };

    world.set_metadata(metadata.clone());

    assert(world.metadata(0) == metadata, 'invalid metadata');
}

#[test]
#[available_gas(60000000)]
fn test_set_metadata_model_writer() {
    let world = spawn_test_world(array![foo::TEST_CLASS_HASH],);

    let bar_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
    };

    world.grant_writer(selector!("Foo"), bar_contract.contract_address);

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bar_contract.contract_address);

    bar_contract.set_foo(1337, 1337);

    let metadata = ResourceMetadata {
        resource_id: selector!("Foo"), metadata_uri: format!("ipfs:bob")
    };

    // A system that has write access on a model should be able to update the metadata.
    // This follows conventional ACL model.
    world.set_metadata(metadata.clone());
    assert(world.metadata(selector!("Foo")) == metadata, 'bad metadata');
}

#[test]
#[available_gas(60000000)]
#[should_panic(expected: ('not writer', 'ENTRYPOINT_FAILED',))]
fn test_set_metadata_same_model_rules() {
    let world = deploy_world();

    let metadata = ResourceMetadata { // World metadata.
        resource_id: 0, metadata_uri: format!("ipfs:bob"),
    };

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_contract_address(bob);
    starknet::testing::set_account_contract_address(bob);

    // Bob access follows the conventional ACL, he can't write the world
    // metadata if he does not have access to it.
    world.set_metadata(metadata);
}

#[test]
#[available_gas(60000000)]
#[should_panic(expected: ('only owner can update', 'ENTRYPOINT_FAILED',))]
fn test_metadata_update_owner_only() {
    let world = deploy_world();

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_contract_address(bob);
    starknet::testing::set_account_contract_address(bob);

    world.register_model(resource_metadata_malicious::TEST_CLASS_HASH.try_into().unwrap());
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
fn test_system_writer_access() {
    let world = spawn_test_world(array![foo::TEST_CLASS_HASH],);

    let bar_address = deploy_with_world_address(bar::TEST_CLASS_HASH, world);
    let bar_contract = IbarDispatcher { contract_address: bar_address };

    world.grant_writer(selector!("Foo"), bar_address);
    assert(world.is_writer(selector!("Foo"), bar_address), 'should be writer');

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

    let gas = GasCounterImpl::start();

    bar_contract.set_foo(1337, 1337);
    gas.end("foo set call");

    let gas = GasCounterImpl::start();
    let data = get!(world, alice, Foo);
    gas.end("foo get macro");

    assert(data.a == 1337, 'data not stored');
}

#[test]
fn bench_execute_complex() {
    let world = spawn_test_world(array![foo::TEST_CLASS_HASH],);
    let bar_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
    };

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);

    let gas = GasCounterImpl::start();

    bar_contract.set_char(1337, 1337);
    gas.end("char set call");

    let gas = GasCounterImpl::start();

    let data = get!(world, alice, Character);
    gas.end("char get macro");

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

    #[abi(embed_v0)]
    impl IWorldUpgradeImpl of super::IWorldUpgrade<ContractState> {
        fn hello(self: @ContractState) -> felt252 {
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

    let res = (IWorldUpgradeDispatcher { contract_address: world.contract_address }).hello();

    assert(res == 'dojo', 'should return dojo');
}

#[test]
#[available_gas(60000000)]
#[should_panic(expected: ('invalid class_hash', 'ENTRYPOINT_FAILED'))]
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
#[should_panic(expected: ('only owner can upgrade', 'ENTRYPOINT_FAILED'))]
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

fn drop_all_events(address: ContractAddress) {
    loop {
        match starknet::testing::pop_log_raw(address) {
            option::Option::Some(_) => {},
            option::Option::None => { break; },
        };
    }
}

#[test]
#[available_gas(6000000)]
fn test_differ_program_hash_event_emit() {
    let world = deploy_world();
    drop_all_events(world.contract_address);
    let config = IConfigDispatcher { contract_address: world.contract_address };

    config.set_differ_program_hash(program_hash: 98758347158781475198374598718743);

    assert_eq!(
        starknet::testing::pop_log(world.contract_address),
        Option::Some(DifferProgramHashUpdate { program_hash: 98758347158781475198374598718743 })
    );
}

#[test]
#[available_gas(6000000)]
fn test_facts_registry_event_emit() {
    let world = deploy_world();
    drop_all_events(world.contract_address);
    let config = IConfigDispatcher { contract_address: world.contract_address };

    config.set_facts_registry(contract_address_const::<0x12>());

    assert_eq!(
        starknet::testing::pop_log(world.contract_address),
        Option::Some(FactsRegistryUpdate { address: contract_address_const::<0x12>() })
    );
}

#[starknet::interface]
trait IDojoInit<ContractState> {
    fn dojo_init(self: @ContractState) -> felt252;
}

#[dojo::contract]
mod test_contract {}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('Only world can init', 'ENTRYPOINT_FAILED'))]
fn test_can_call_init() {
    let world = deploy_world();
    let address = world
        .deploy_contract(
            'salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap(), array![].span()
        );

    let dojo_init = IDojoInitDispatcher { contract_address: address };
    dojo_init.dojo_init();
}

#[test]
fn test_set_entity_with_fixed_layout() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("foo");
    let keys = get_key_test();
    let values = create_foo();
    let layout = dojo::model::Model::<Foo>::layout();

    world.set_entity(selector, get_key_test(), values, layout);

    let read_values = world.entity(selector, keys, layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_simple_model");
    let keys = get_key_test();
    let values = create_struct_simple_model();
    let layout = dojo::model::Model::<StructSimpleModel>::layout();

    world.set_entity(selector, keys, values, layout);

    let read_values = world.entity(selector, keys, layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_tuple_layout() {
    let world = deploy_world();
    world.register_model(struct_with_tuple::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_with_tuple");
    let keys = get_key_test();
    let values = create_struct_with_tuple();
    let layout = dojo::model::Model::<StructWithTuple>::layout();

    world.set_entity(selector, keys, values, layout);

    let read_values = world.entity(selector, keys, layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_enum_layout() {
    let world = deploy_world();
    world.register_model(struct_with_enum::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_with_enum");
    let keys = get_key_test();
    let values = create_struct_with_enum_first_variant();
    let layout = dojo::model::Model::<StructWithEnum>::layout();

    // test with the first variant
    world.set_entity(selector, keys, values, layout);

    let read_values = world.entity(selector, keys, layout);
    assert_array(read_values, values);

    // then override with the second variant
    let values = create_struct_with_enum_second_variant();
    world.set_entity(selector, keys, values, layout);

    let read_values = world.entity(selector, keys, layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_simple_array_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_simple_array_model");
    let keys = get_key_test();
    let values = create_struct_simple_array_model();
    let layout = dojo::model::Model::<StructSimpleArrayModel>::layout();

    world.set_entity(selector, keys, values, layout);

    let read_values = world.entity(selector, keys, layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_complex_array_layout() {
    let world = deploy_world();
    world.register_model(struct_complex_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_complex_array_model");
    let keys = get_key_test();
    let values = create_struct_complex_array_model();
    let layout = dojo::model::Model::<StructComplexArrayModel>::layout();

    world.set_entity(selector, keys, values, layout);

    let read_values = world.entity(selector, keys, layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_layout_and_byte_array() {
    let world = deploy_world();
    world.register_model(struct_byte_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_byte_array_model");
    let keys = get_key_test();
    let values = create_struct_byte_array_model();
    let layout = dojo::model::Model::<StructByteArrayModel>::layout();

    world.set_entity(selector, keys, values, layout);

    let read_values = world.entity(selector, keys, layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_nested_elements() {
    let world = deploy_world();
    world.register_model(struct_nested_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_nested_model");
    let keys = get_key_test();
    let values = create_struct_nested_model();
    let layout = dojo::model::Model::<StructNestedModel>::layout();

    world.set_entity(selector, keys, values, layout);

    let read_values = world.entity(selector, keys, layout);
    assert_array(read_values, values);
}

fn assert_empty_array(values: Span<felt252>) {
    let mut i = 0;
    loop {
        if i >= values.len() {
            break;
        }
        assert!(*values.at(i) == 0);
        i += 1;
    };
}

#[test]
fn test_set_entity_with_struct_generics_enum_layout() {
    let world = deploy_world();
    world.register_model(struct_with_generic::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_with_generic");
    let keys = get_key_test();
    let values = create_struct_generic_first_variant();
    let layout = dojo::model::Model::<StructWithGeneric>::layout();

    // test with the first variant
    world.set_entity(selector, keys, values, layout);

    let read_values = world.entity(selector, keys, layout);
    assert_array(read_values, values);

    // then override with the second variant
    let values = create_struct_generic_second_variant();
    world.set_entity(selector, keys, values, layout);

    let read_values = world.entity(selector, keys, layout);
    assert_array(read_values, values);
}

#[test]
fn test_delete_entity_with_fixed_layout() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("foo");
    let keys = get_key_test();
    let values = create_foo();
    let layout = dojo::model::Model::<Foo>::layout();

    world.set_entity(selector, keys, values, layout);

    world.delete_entity(selector, keys, layout);

    let read_values = world.entity(selector, keys, layout);

    assert!(read_values.len() == values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_simple_struct_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_simple_model");
    let keys = get_key_test();
    let values = create_struct_simple_model();
    let layout = dojo::model::Model::<StructSimpleModel>::layout();

    world.set_entity(selector, keys, values, layout);

    world.delete_entity(selector, keys, layout);

    let read_values = world.entity(selector, keys, layout);

    assert!(read_values.len() == values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_simple_array_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_simple_array_model");
    let keys = get_key_test();
    let values = create_struct_simple_array_model();
    let layout = dojo::model::Model::<StructSimpleArrayModel>::layout();

    world.set_entity(selector, keys, values, layout);

    world.delete_entity(selector, keys, layout);

    let read_values = world.entity(selector, keys, layout);

    // array length set to 0, so the expected value span is shorter than the initial values
    let expected_values = array![0, 0, 0].span();

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_complex_array_struct_layout() {
    let world = deploy_world();
    world.register_model(struct_complex_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_complex_array_model");
    let keys = get_key_test();
    let values = create_struct_complex_array_model();

    let layout = dojo::model::Model::<StructComplexArrayModel>::layout();

    world.set_entity(selector, keys, values, layout);

    world.delete_entity(selector, keys, layout);

    let read_values = world.entity(selector, keys, layout);

    // array length set to 0, so the expected value span is shorter than the initial values
    let expected_values = array![0, 0, 0, 0, 0, 0, 0, 0, 0, 0].span();

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_tuple_layout() {
    let world = deploy_world();
    world.register_model(struct_with_tuple::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_with_tuple");
    let keys = get_key_test();
    let values = create_struct_with_tuple();
    let layout = dojo::model::Model::<StructWithTuple>::layout();

    world.set_entity(selector, keys, values, layout);

    world.delete_entity(selector, keys, layout);

    let expected_values = array![0, 0].span();
    let read_values = world.entity(selector, keys, layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_enum_layout() {
    let world = deploy_world();
    world.register_model(struct_with_enum::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_with_enum");
    let keys = get_key_test();
    let values = create_struct_with_enum_first_variant();
    let layout = dojo::model::Model::<StructWithEnum>::layout();

    // test with the first variant
    world.set_entity(selector, keys, values, layout);

    world.delete_entity(selector, keys, layout);

    let expected_values = array![0, 0, 0].span();
    let read_values = world.entity(selector, keys, layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_layout_and_byte_array() {
    let world = deploy_world();
    world.register_model(struct_byte_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_byte_array_model");
    let keys = get_key_test();
    let values = create_struct_byte_array_model();
    let layout = dojo::model::Model::<StructByteArrayModel>::layout();

    world.set_entity(selector, keys, values, layout);

    world.delete_entity(selector, keys, layout);

    let expected_values = array![0, 0, 0, 0].span();
    let read_values = world.entity(selector, keys, layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_nested_elements() {
    let world = deploy_world();
    world.register_model(struct_nested_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_nested_model");
    let keys = get_key_test();
    let values = create_struct_nested_model();
    let layout = dojo::model::Model::<StructNestedModel>::layout();

    world.set_entity(selector, keys, values, layout);

    world.delete_entity(selector, keys, layout);

    let expected_values = array![0, 0, 0, 0, 0, 0, 0, 0, 0].span();
    let read_values = world.entity(selector, keys, layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_generics_enum_layout() {
    let world = deploy_world();
    world.register_model(struct_with_generic::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_with_generic");
    let keys = get_key_test();
    let values = create_struct_generic_first_variant();
    let layout = dojo::model::Model::<StructWithGeneric>::layout();

    world.set_entity(selector, keys, values, layout);

    world.delete_entity(selector, keys, layout);

    let expected_values = array![0, 0].span();
    let read_values = world.entity(selector, keys, layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_set_entity_with_unexpected_array_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Array(
        array![dojo::database::introspect::Introspect::<felt252>::layout()].span()
    );

    world
        .set_entity(
            selector!("struct_simple_array_model"), array![].span(), array![].span(), layout
        );
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_set_entity_with_unexpected_tuple_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Tuple(
        array![dojo::database::introspect::Introspect::<felt252>::layout()].span()
    );

    world
        .set_entity(
            selector!("struct_simple_array_model"), array![].span(), array![].span(), layout
        );
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_delete_entity_with_unexpected_array_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Array(
        array![dojo::database::introspect::Introspect::<felt252>::layout()].span()
    );

    world.delete_entity(selector!("struct_simple_array_model"), array![].span(), layout);
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_delete_entity_with_unexpected_tuple_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Tuple(
        array![dojo::database::introspect::Introspect::<felt252>::layout()].span()
    );

    world.delete_entity(selector!("struct_simple_array_model"), array![].span(), layout);
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_get_entity_with_unexpected_array_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Array(
        array![dojo::database::introspect::Introspect::<felt252>::layout()].span()
    );

    world.entity(selector!("struct_simple_array_model"), array![].span(), layout);
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_get_entity_with_unexpected_tuple_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Tuple(
        array![dojo::database::introspect::Introspect::<felt252>::layout()].span()
    );

    world.entity(selector!("struct_simple_array_model"), array![].span(), layout);
}


#[test]
#[should_panic(expected: ('Invalid values length', 'ENTRYPOINT_FAILED',))]
fn test_set_entity_with_bad_values_length_error_for_array_layout() {
    let world = deploy_world();

    let selector = selector!("a_selector");
    let keys = get_key_test();
    let layout = Layout::Struct(
        array![
            FieldLayout {
                selector: selector!("a"),
                layout: Layout::Array(
                    array![dojo::database::introspect::Introspect::<felt252>::layout()].span()
                )
            },
        ]
            .span()
    );

    world.set_entity(selector, keys, array![].span(), layout);
}

#[test]
#[should_panic(expected: ('invalid array length', 'ENTRYPOINT_FAILED',))]
fn test_set_entity_with_too_big_array_length() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_simple_array_model");
    let keys = get_key_test();
    let values: Span<felt252> = array![
        1, MAX_ARRAY_LENGTH.try_into().unwrap() + 1, 10, 20, 30, 40, 2
    ]
        .span();
    let layout = dojo::model::Model::<StructSimpleArrayModel>::layout();

    world.set_entity(selector, keys, values, layout);
}

#[test]
#[should_panic(expected: ('invalid array length', 'ENTRYPOINT_FAILED',))]
fn test_set_entity_with_struct_layout_and_bad_byte_array_length() {
    let world = deploy_world();
    world.register_model(struct_byte_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_byte_array_model");
    let keys = get_key_test();
    let values: Span<felt252> = array![
        1, MAX_ARRAY_LENGTH.try_into().unwrap(), 'first', 'second', 'third', 'pending', 7
    ]
        .span();
    let layout = dojo::model::Model::<StructByteArrayModel>::layout();

    world.set_entity(selector, keys, values, layout);
}

#[test]
#[should_panic(expected: ('Invalid values length', 'ENTRYPOINT_FAILED',))]
fn test_set_entity_with_struct_layout_and_bad_value_length_for_byte_array() {
    let world = deploy_world();
    world.register_model(struct_byte_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = selector!("struct_byte_array_model");
    let keys = get_key_test();
    let values: Span<felt252> = array![1, 3, 'first', 'second', 'third', 'pending'].span();
    let layout = dojo::model::Model::<StructByteArrayModel>::layout();

    world.set_entity(selector, keys, values, layout);
}

use core::array::{ArrayTrait, SpanTrait};
use core::clone::Clone;
use core::option::OptionTrait;
use core::result::ResultTrait;
use core::traits::{Into, TryInto};

use starknet::{contract_address_const, ContractAddress, ClassHash, get_caller_address};
use starknet::syscalls::deploy_syscall;

use dojo::world::config::Config::{
    DifferProgramHashUpdate, MergerProgramHashUpdate, FactsRegistryUpdate
};
use dojo::world::config::{IConfigDispatcher, IConfigDispatcherTrait};
use dojo::model::{ModelIndex, Layout, FieldLayout, Model, ResourceMetadata};
use dojo::model::introspect::{Introspect};
use dojo::utils::bytearray_hash;
use dojo::storage::database::MAX_ARRAY_LENGTH;
use dojo::utils::test::{spawn_test_world, deploy_with_world_address, assert_array, GasCounterTrait};
use dojo::utils::entity_id_from_keys;
use dojo::world::{
    IWorldDispatcher, IWorldDispatcherTrait, world, IUpgradeableWorld, IUpgradeableWorldDispatcher,
    IUpgradeableWorldDispatcherTrait
};
use dojo::world::world::NamespaceRegistered;

use super::benchmarks;
use super::benchmarks::Character;

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
pub struct Foo {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Copy, Drop, Serde)]
#[dojo::model(namespace: "another_namespace", nomapping: true)]
pub struct Buzz {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}


fn create_foo() -> Span<felt252> {
    array![1, 2].span()
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
pub struct Fizz {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
pub struct StructSimpleModel {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

fn create_struct_simple_model() -> Span<felt252> {
    array![1, 2].span()
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
pub struct StructWithTuple {
    #[key]
    pub caller: ContractAddress,
    pub a: (u8, u64)
}

fn create_struct_with_tuple() -> Span<felt252> {
    array![12, 58].span()
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
pub struct StructWithEnum {
    #[key]
    pub caller: ContractAddress,
    pub a: OneEnum,
}

fn create_struct_with_enum_first_variant() -> Span<felt252> {
    array![0, 1, 2].span()
}

fn create_struct_with_enum_second_variant() -> Span<felt252> {
    array![1].span()
}

#[derive(Copy, Drop, Serde)]
#[dojo::model]
pub struct StructSimpleArrayModel {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: Array<u64>,
    pub c: u128,
}

impl ArrayU64Copy of core::traits::Copy<Array<u64>>;

fn create_struct_simple_array_model() -> Span<felt252> {
    array![1, 4, 10, 20, 30, 40, 2].span()
}

#[derive(Drop, Serde)]
#[dojo::model]
pub struct StructByteArrayModel {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: ByteArray,
}

fn create_struct_byte_array_model() -> Span<felt252> {
    array![1, 3, 'first', 'second', 'third', 'pending', 7].span()
}

#[derive(Introspect, Copy, Drop, Serde)]
pub struct ModelData {
    pub x: u256,
    pub y: u32,
    pub z: felt252
}

#[derive(Drop, Serde)]
#[dojo::model]
pub struct StructComplexArrayModel {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: Array<ModelData>,
    pub c: AnotherEnum,
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
pub struct StructNestedModel {
    #[key]
    pub caller: ContractAddress,
    pub x: (u8, u16, (u32, ByteArray, u8), Array<(u8, u16)>),
    pub y: Array<Array<(u8, (u16, u256))>>
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
pub enum EnumGeneric<T, U> {
    One: T,
    Two: U
}

#[derive(Drop, Serde)]
#[dojo::model]
pub struct StructWithGeneric {
    #[key]
    pub caller: ContractAddress,
    pub x: EnumGeneric<u8, u256>,
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
    fn namespace(self: @T) -> ByteArray;
    fn namespace_hash(self: @T) -> felt252;
}

#[starknet::contract]
mod resource_metadata_malicious {
    use dojo::model::{Model, ResourceMetadata};
    use dojo::utils::bytearray_hash;

    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl InvalidModelName of super::IMetadataOnly<ContractState> {
        fn selector(self: @ContractState) -> felt252 {
            Model::<ResourceMetadata>::selector()
        }

        fn namespace(self: @ContractState) -> ByteArray {
            "dojo"
        }

        fn namespace_hash(self: @ContractState) -> felt252 {
            bytearray_hash(@Self::namespace(self))
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
    use core::traits::Into;
    use starknet::{get_caller_address, ContractAddress};
    use starknet::storage::{StoragePointerReadAccess, StoragePointerWriteAccess};
    use dojo::model::{Model, ModelIndex};

    use super::{Foo, IWorldDispatcher, IWorldDispatcherTrait, Introspect};
    use super::benchmarks::{Character, Abilities, Stats, Weapon, Sword};

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
                    Model::<Foo>::selector(),
                    ModelIndex::Keys(array![get_caller_address().into()].span()),
                    Model::<Foo>::layout()
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

#[test]
#[available_gas(6000000)]
fn test_contract_getter() {
    let world = deploy_world();

    let _ = world
        .deploy_contract(
            'salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap(), array![].span()
        );

    let (class_hash, _) = world.contract(selector_from_tag!("dojo-test_contract"));
    assert(
        class_hash == test_contract::TEST_CLASS_HASH.try_into().unwrap(),
        'invalid contract class hash'
    );
}

#[test]
#[available_gas(6000000)]
fn test_model_class_hash_getter() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let (foo_class_hash, _) = world.model(Model::<Foo>::selector());
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
fn test_register_namespace() {
    let world = deploy_world();

    let caller = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(caller);

    drop_all_events(world.contract_address);

    let namespace = "namespace";
    let hash = bytearray_hash(@namespace);

    world.register_namespace(namespace);

    assert(world.is_owner(hash, caller), 'namespace not registered');

    assert_eq!(
        starknet::testing::pop_log(world.contract_address),
        Option::Some(NamespaceRegistered { namespace: "namespace", hash })
    );
}

#[test]
fn test_register_namespace_already_registered_same_caller() {
    let world = deploy_world();

    let caller = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(caller);

    let namespace = "namespace";
    let hash = bytearray_hash(@namespace);

    world.register_namespace(namespace);

    drop_all_events(world.contract_address);

    world.register_namespace("namespace");

    assert(world.is_owner(hash, caller), 'namespace not registered');

    let event = starknet::testing::pop_log_raw(world.contract_address);
    assert(event.is_none(), 'unexpected event');
}

#[test]
#[should_panic(expected: ('namespace already registered', 'ENTRYPOINT_FAILED',))]
fn test_register_namespace_already_registered_other_caller() {
    let world = deploy_world();

    let account = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(account);

    world.register_namespace("namespace");

    let another_account = starknet::contract_address_const::<0xa11ce>();
    starknet::testing::set_account_contract_address(another_account);

    world.register_namespace("namespace");
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
    spawn_test_world("dojo", array![])
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
    let world = spawn_test_world("dojo", array![foo::TEST_CLASS_HASH],);

    let bar_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
    };

    world.grant_writer(Model::<Foo>::selector(), bar_contract.contract_address);

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bar_contract.contract_address);

    bar_contract.set_foo(1337, 1337);

    let metadata = ResourceMetadata {
        resource_id: Model::<Foo>::selector(), metadata_uri: format!("ipfs:bob")
    };

    // A system that has write access on a model should be able to update the metadata.
    // This follows conventional ACL model.
    world.set_metadata(metadata.clone());
    assert(world.metadata(Model::<Foo>::selector()) == metadata, 'bad metadata');
}

#[test]
#[available_gas(60000000)]
#[should_panic(expected: ('no write access', 'ENTRYPOINT_FAILED',))]
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

    world.grant_owner(bytearray_hash(@"dojo"), bob);

    starknet::testing::set_account_contract_address(bob);

    world.register_model(resource_metadata_malicious::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[available_gas(6000000)]
fn test_owner() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let foo_selector = Model::<Foo>::selector();

    let alice = starknet::contract_address_const::<0x1337>();
    let bob = starknet::contract_address_const::<0x1338>();

    assert(!world.is_owner(0, alice), 'should not be owner');
    assert(!world.is_owner(foo_selector, bob), 'should not be owner');

    world.grant_owner(0, alice);
    assert(world.is_owner(0, alice), 'should be owner');

    world.grant_owner(foo_selector, bob);
    assert(world.is_owner(foo_selector, bob), 'should be owner');

    world.revoke_owner(0, alice);
    assert(!world.is_owner(0, alice), 'should not be owner');

    world.revoke_owner(foo_selector, bob);
    assert(!world.is_owner(foo_selector, bob), 'should not be owner');
}

#[test]
#[available_gas(6000000)]
#[should_panic]
fn test_set_owner_fails_for_non_owner() {
    let world = deploy_world();

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_account_contract_address(alice);

    world.revoke_owner(0, alice);
    assert(!world.is_owner(0, alice), 'should not be owner');

    world.grant_owner(0, alice);
}

#[test]
#[available_gas(6000000)]
fn test_writer() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let foo_selector = Model::<Foo>::selector();

    assert(!world.is_writer(foo_selector, 69.try_into().unwrap()), 'should not be writer');

    world.grant_writer(foo_selector, 69.try_into().unwrap());
    assert(world.is_writer(foo_selector, 69.try_into().unwrap()), 'should be writer');

    world.revoke_writer(foo_selector, 69.try_into().unwrap());
    assert(!world.is_writer(foo_selector, 69.try_into().unwrap()), 'should not be writer');
}

#[test]
#[should_panic(expected: ('resource not registered', 'ENTRYPOINT_FAILED'))]
fn test_writer_not_registered_resource() {
    let world = deploy_world();

    // 42 is not a registered resource ID
    world.grant_writer(42, 69.try_into().unwrap());
}

#[test]
#[available_gas(6000000)]
#[should_panic]
fn test_system_not_writer_fail() {
    let world = spawn_test_world("dojo", array![foo::TEST_CLASS_HASH],);

    let bar_address = deploy_with_world_address(bar::TEST_CLASS_HASH, world);
    let bar_contract = IbarDispatcher { contract_address: bar_address };

    // Caller is not owner now
    let account = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(account);

    // Should panic, system not writer
    bar_contract.set_foo(25, 16);
}

#[test]
fn test_system_writer_access() {
    let world = spawn_test_world("dojo", array![foo::TEST_CLASS_HASH],);

    let bar_address = deploy_with_world_address(bar::TEST_CLASS_HASH, world);
    let bar_contract = IbarDispatcher { contract_address: bar_address };

    world.grant_writer(Model::<Foo>::selector(), bar_address);
    assert(world.is_writer(Model::<Foo>::selector(), bar_address), 'should be writer');

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

    assert(!world.is_owner(0, alice), 'should not be owner');

    world.grant_writer(42, 69.try_into().unwrap());
}

#[test]
fn test_execute_multiple_worlds() {
    // Deploy world contract
    let world1 = spawn_test_world("dojo", array![foo::TEST_CLASS_HASH],);

    let bar1_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world1)
    };

    // Deploy another world contract
    let world2 = spawn_test_world("dojo", array![foo::TEST_CLASS_HASH],);

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
    let world = spawn_test_world("dojo", array![foo::TEST_CLASS_HASH],);
    let bar_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
    };

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);

    let gas = GasCounterTrait::start();

    bar_contract.set_foo(1337, 1337);
    gas.end("foo set call");

    let gas = GasCounterTrait::start();
    let data = get!(world, alice, Foo);
    gas.end("foo get macro");

    assert(data.a == 1337, 'data not stored');
}

#[test]
fn bench_execute_complex() {
    let world = spawn_test_world("dojo", array![benchmarks::character::TEST_CLASS_HASH],);
    let bar_contract = IbarDispatcher {
        contract_address: deploy_with_world_address(bar::TEST_CLASS_HASH, world)
    };

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);

    let gas = GasCounterTrait::start();

    bar_contract.set_char(1337, 1337);
    gas.end("char set call");

    let gas = GasCounterTrait::start();

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
    use starknet::storage::{StoragePointerReadAccess, StoragePointerWriteAccess};

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
            core::option::Option::Some(_) => {},
            core::option::Option::None => { break; },
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

#[dojo::contract(namespace: "buzz_namespace", nomapping: true)]
mod buzz_contract {}

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
fn test_set_entity_by_id() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let selector = Model::<Foo>::selector();
    let entity_id = entity_id_from_keys(array![0x01234].span());
    let values = create_foo();
    let layout = Model::<Foo>::layout();

    world.set_entity(selector, ModelIndex::Id(entity_id), values, layout);
    let read_values = world.entity(selector, ModelIndex::Id(entity_id), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_fixed_layout() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let selector = Model::<Foo>::selector();
    let keys = get_key_test();
    let values = create_foo();
    let layout = Model::<Foo>::layout();

    world.set_entity(selector, ModelIndex::Keys(get_key_test()), values, layout);
    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructSimpleModel>::selector();
    let keys = get_key_test();
    let values = create_struct_simple_model();
    let layout = Model::<StructSimpleModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_tuple_layout() {
    let world = deploy_world();
    world.register_model(struct_with_tuple::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructWithTuple>::selector();
    let keys = get_key_test();
    let values = create_struct_with_tuple();
    let layout = Model::<StructWithTuple>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_enum_layout() {
    let world = deploy_world();
    world.register_model(struct_with_enum::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructWithEnum>::selector();
    let keys = get_key_test();
    let values = create_struct_with_enum_first_variant();
    let layout = Model::<StructWithEnum>::layout();

    // test with the first variant
    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);

    // then override with the second variant
    let values = create_struct_with_enum_second_variant();
    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_simple_array_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructSimpleArrayModel>::selector();
    let keys = get_key_test();
    let values = create_struct_simple_array_model();
    let layout = Model::<StructSimpleArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_complex_array_layout() {
    let world = deploy_world();
    world.register_model(struct_complex_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructComplexArrayModel>::selector();
    let keys = get_key_test();
    let values = create_struct_complex_array_model();
    let layout = Model::<StructComplexArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_struct_layout_and_byte_array() {
    let world = deploy_world();
    world.register_model(struct_byte_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructByteArrayModel>::selector();
    let keys = get_key_test();
    let values = create_struct_byte_array_model();
    let layout = Model::<StructByteArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_set_entity_with_nested_elements() {
    let world = deploy_world();
    world.register_model(struct_nested_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructNestedModel>::selector();
    let keys = get_key_test();
    let values = create_struct_nested_model();
    let layout = Model::<StructNestedModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
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

    let selector = Model::<StructWithGeneric>::selector();
    let keys = get_key_test();
    let values = create_struct_generic_first_variant();
    let layout = Model::<StructWithGeneric>::layout();

    // test with the first variant
    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);

    // then override with the second variant
    let values = create_struct_generic_second_variant();
    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);
    assert_array(read_values, values);
}

#[test]
fn test_delete_entity_by_id() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let selector = Model::<Foo>::selector();
    let entity_id = entity_id_from_keys(get_key_test());
    let values = create_foo();
    let layout = Model::<Foo>::layout();

    world.set_entity(selector, ModelIndex::Id(entity_id), values, layout);

    world.delete_entity(selector, ModelIndex::Id(entity_id), layout);

    let read_values = world.entity(selector, ModelIndex::Id(entity_id), layout);

    assert!(read_values.len() == values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_fixed_layout() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let selector = Model::<Foo>::selector();
    let keys = get_key_test();
    let values = create_foo();
    let layout = Model::<Foo>::layout();

    world.set_entity(selector, ModelIndex::Keys(get_key_test()), values, layout);

    world.delete_entity(selector, ModelIndex::Keys(keys), layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_simple_struct_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructSimpleModel>::selector();
    let keys = get_key_test();
    let values = create_struct_simple_model();
    let layout = Model::<StructSimpleModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    world.delete_entity(selector, ModelIndex::Keys(keys), layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_simple_array_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructSimpleArrayModel>::selector();
    let keys = get_key_test();
    let values = create_struct_simple_array_model();
    let layout = Model::<StructSimpleArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    world.delete_entity(selector, ModelIndex::Keys(keys), layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    // array length set to 0, so the expected value span is shorter than the initial values
    let expected_values = array![0, 0, 0].span();

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_complex_array_struct_layout() {
    let world = deploy_world();
    world.register_model(struct_complex_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructComplexArrayModel>::selector();
    let keys = get_key_test();
    let values = create_struct_complex_array_model();

    let layout = Model::<StructComplexArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    world.delete_entity(selector, ModelIndex::Keys(keys), layout);

    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    // array length set to 0, so the expected value span is shorter than the initial values
    let expected_values = array![0, 0, 0, 0, 0, 0, 0, 0, 0, 0].span();

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_tuple_layout() {
    let world = deploy_world();
    world.register_model(struct_with_tuple::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructWithTuple>::selector();
    let keys = get_key_test();
    let values = create_struct_with_tuple();
    let layout = Model::<StructWithTuple>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    world.delete_entity(selector, ModelIndex::Keys(keys), layout);

    let expected_values = array![0, 0].span();
    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_enum_layout() {
    let world = deploy_world();
    world.register_model(struct_with_enum::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructWithEnum>::selector();
    let keys = get_key_test();
    let values = create_struct_with_enum_first_variant();
    let layout = Model::<StructWithEnum>::layout();

    // test with the first variant
    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    world.delete_entity(selector, ModelIndex::Keys(keys), layout);

    let expected_values = array![0, 0, 0].span();
    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_layout_and_byte_array() {
    let world = deploy_world();
    world.register_model(struct_byte_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructByteArrayModel>::selector();
    let keys = get_key_test();
    let values = create_struct_byte_array_model();
    let layout = Model::<StructByteArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    world.delete_entity(selector, ModelIndex::Keys(keys), layout);

    let expected_values = array![0, 0, 0, 0].span();
    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_nested_elements() {
    let world = deploy_world();
    world.register_model(struct_nested_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructNestedModel>::selector();
    let keys = get_key_test();
    let values = create_struct_nested_model();
    let layout = Model::<StructNestedModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    world.delete_entity(selector, ModelIndex::Keys(keys), layout);

    let expected_values = array![0, 0, 0, 0, 0, 0, 0, 0, 0].span();
    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
fn test_delete_entity_with_struct_generics_enum_layout() {
    let world = deploy_world();
    world.register_model(struct_with_generic::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructWithGeneric>::selector();
    let keys = get_key_test();
    let values = create_struct_generic_first_variant();
    let layout = Model::<StructWithGeneric>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);

    world.delete_entity(selector, ModelIndex::Keys(keys), layout);

    let expected_values = array![0, 0].span();
    let read_values = world.entity(selector, ModelIndex::Keys(keys), layout);

    assert!(read_values.len() == expected_values.len());
    assert_empty_array(read_values);
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_set_entity_with_unexpected_array_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Array(array![Introspect::<felt252>::layout()].span());

    world
        .set_entity(
            Model::<StructSimpleArrayModel>::selector(),
            ModelIndex::Keys(array![].span()),
            array![].span(),
            layout
        );
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_set_entity_with_unexpected_tuple_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Tuple(array![Introspect::<felt252>::layout()].span());

    world
        .set_entity(
            Model::<StructSimpleArrayModel>::selector(),
            ModelIndex::Keys(array![].span()),
            array![].span(),
            layout
        );
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_delete_entity_with_unexpected_array_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Array(array![Introspect::<felt252>::layout()].span());

    world
        .delete_entity(
            Model::<StructSimpleArrayModel>::selector(), ModelIndex::Keys(array![].span()), layout
        );
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_delete_entity_with_unexpected_tuple_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Tuple(array![Introspect::<felt252>::layout()].span());

    world
        .delete_entity(
            Model::<StructSimpleArrayModel>::selector(), ModelIndex::Keys(array![].span()), layout
        );
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_get_entity_with_unexpected_array_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Array(array![Introspect::<felt252>::layout()].span());

    world
        .entity(
            Model::<StructSimpleArrayModel>::selector(), ModelIndex::Keys(array![].span()), layout
        );
}

#[test]
#[should_panic(expected: ("Unexpected layout type for a model.", 'ENTRYPOINT_FAILED'))]
fn test_get_entity_with_unexpected_tuple_model_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let layout = Layout::Tuple(array![Introspect::<felt252>::layout()].span());

    world
        .entity(
            Model::<StructSimpleArrayModel>::selector(), ModelIndex::Keys(array![].span()), layout
        );
}


#[test]
#[should_panic(expected: ('Invalid values length', 'ENTRYPOINT_FAILED',))]
fn test_set_entity_with_bad_values_length_error_for_array_layout() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructSimpleArrayModel>::selector();
    let keys = get_key_test();
    let layout = Model::<StructSimpleArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), array![1].span(), layout);
}

#[test]
#[should_panic(expected: ('invalid array length', 'ENTRYPOINT_FAILED',))]
fn test_set_entity_with_too_big_array_length() {
    let world = deploy_world();
    world.register_model(struct_simple_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructSimpleArrayModel>::selector();
    let keys = get_key_test();
    let values: Span<felt252> = array![
        1, MAX_ARRAY_LENGTH.try_into().unwrap() + 1, 10, 20, 30, 40, 2
    ]
        .span();
    let layout = Model::<StructSimpleArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);
}

#[test]
#[should_panic(expected: ('invalid array length', 'ENTRYPOINT_FAILED',))]
fn test_set_entity_with_struct_layout_and_bad_byte_array_length() {
    let world = deploy_world();
    world.register_model(struct_byte_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructByteArrayModel>::selector();
    let keys = get_key_test();
    let values: Span<felt252> = array![
        1, MAX_ARRAY_LENGTH.try_into().unwrap(), 'first', 'second', 'third', 'pending', 7
    ]
        .span();
    let layout = Model::<StructByteArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);
}

#[test]
#[should_panic(expected: ('Invalid values length', 'ENTRYPOINT_FAILED',))]
fn test_set_entity_with_struct_layout_and_bad_value_length_for_byte_array() {
    let world = deploy_world();
    world.register_model(struct_byte_array_model::TEST_CLASS_HASH.try_into().unwrap());

    let selector = Model::<StructByteArrayModel>::selector();
    let keys = get_key_test();
    let values: Span<felt252> = array![1, 3, 'first', 'second', 'third', 'pending'].span();
    let layout = Model::<StructByteArrayModel>::layout();

    world.set_entity(selector, ModelIndex::Keys(keys), values, layout);
}

fn write_foo_record(world: IWorldDispatcher) {
    let selector = Model::<Foo>::selector();
    let values = create_foo();
    let layout = Model::<Foo>::layout();

    world.set_entity(selector, ModelIndex::Keys(get_key_test()), values, layout);
}

#[test]
fn test_write_model_for_namespace_owner() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let account = starknet::contract_address_const::<0xb0b>();
    let contract = starknet::contract_address_const::<0xdeadbeef>();

    // the caller account is a model namespace owner
    world.grant_owner(Model::<Foo>::namespace_hash(), account);
    starknet::testing::set_account_contract_address(account);
    starknet::testing::set_contract_address(contract);

    write_foo_record(world);
}

#[test]
fn test_write_model_for_model_owner() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    // the caller account is a model owner
    let account = starknet::contract_address_const::<0xb0b>();
    let contract = starknet::contract_address_const::<0xdeadbeef>();

    world.grant_owner(Model::<Foo>::selector(), account);
    starknet::testing::set_account_contract_address(account);
    starknet::testing::set_contract_address(contract);

    write_foo_record(world);
}

#[test]
fn test_write_model_for_namespace_writer() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let account = starknet::contract_address_const::<0xb0b>();
    let contract = starknet::contract_address_const::<0xdeadbeef>();

    world.grant_writer(Model::<Foo>::namespace_hash(), contract);

    // the account does not own anything
    starknet::testing::set_account_contract_address(account);
    starknet::testing::set_contract_address(contract);

    write_foo_record(world);
}

#[test]
fn test_write_model_for_model_writer() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let account = starknet::contract_address_const::<0xb0b>();
    let contract = starknet::contract_address_const::<0xdeadbeef>();

    world.grant_writer(Model::<Foo>::selector(), contract);

    // the account does not own anything
    starknet::testing::set_account_contract_address(account);
    starknet::testing::set_contract_address(contract);

    write_foo_record(world);
}

#[test]
fn test_write_namespace_for_namespace_owner() {
    let world = deploy_world();

    let account = starknet::contract_address_const::<0xb0b>();
    let contract = starknet::contract_address_const::<0xdeadbeef>();

    world.grant_owner(Model::<Foo>::namespace_hash(), account);

    // the account owns the Foo model namespace so it should be able to deploy
    // and register the model.
    starknet::testing::set_account_contract_address(account);
    starknet::testing::set_contract_address(contract);

    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
fn test_write_namespace_for_namespace_writer() {
    let world = deploy_world();

    let account = starknet::contract_address_const::<0xb0b>();
    let contract = starknet::contract_address_const::<0xdeadbeef>();

    world.grant_writer(Model::<Foo>::namespace_hash(), account);

    // the account has write access to the Foo model namespace so it should be able
    // to deploy and register the model.
    starknet::testing::set_account_contract_address(account);
    starknet::testing::set_contract_address(contract);

    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(expected: ('no model write access', 'ENTRYPOINT_FAILED',))]
fn test_write_model_no_write_access() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    // the caller account does not own the model nor the model namespace nor the world
    let account = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(account);

    // the contract is not a writer for the model nor for the model namespace
    let contract = starknet::contract_address_const::<0xdeadbeef>();
    starknet::testing::set_contract_address(contract);

    write_foo_record(world);
}

#[test]
#[should_panic(expected: ('namespace not registered', 'ENTRYPOINT_FAILED',))]
fn test_register_model_with_unregistered_namespace() {
    let world = deploy_world();
    world.register_model(buzz::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
fn test_deploy_contract_for_namespace_owner() {
    let world = deploy_world();

    let account = starknet::contract_address_const::<0xb0b>();
    world.grant_owner(bytearray_hash(@"dojo"), account);

    // the account owns the 'test_contract' namespace so it should be able to deploy
    // and register the model.
    starknet::testing::set_account_contract_address(account);

    world
        .deploy_contract(
            'salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap(), array![].span()
        );
}

#[test]
fn test_deploy_contract_for_namespace_writer() {
    let world = deploy_world();

    let account = starknet::contract_address_const::<0xb0b>();
    world.grant_writer(bytearray_hash(@"dojo"), account);

    // the account has write access to the 'test_contract' namespace so it should be able
    // to deploy and register the model.
    starknet::testing::set_account_contract_address(account);

    world
        .deploy_contract(
            'salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap(), array![].span()
        );
}

#[test]
#[should_panic(expected: ('namespace not registered', 'ENTRYPOINT_FAILED',))]
fn test_deploy_contract_with_unregistered_namespace() {
    let world = deploy_world();
    world
        .deploy_contract(
            'salt1', buzz_contract::TEST_CLASS_HASH.try_into().unwrap(), array![].span()
        );
}

#[test]
#[should_panic(expected: ('no namespace write access', 'ENTRYPOINT_FAILED',))]
fn test_deploy_contract_no_namespace_write_access() {
    let world = deploy_world();

    let account = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(account);

    world
        .deploy_contract(
            'salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap(), array![].span()
        );
}


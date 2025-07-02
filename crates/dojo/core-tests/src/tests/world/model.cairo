use dojo::model::{Model, ModelStorage};
use dojo::world::{IWorldDispatcherTrait, world};
use dojo_snf_test;
use snforge_std::{EventSpyTrait, EventsFilterTrait, spy_events};
use starknet::ContractAddress;
use crate::tests::helpers::{DOJO_NSH, Foo, MyEnum, deploy_world, deploy_world_for_model_upgrades};

#[dojo::model]
pub struct FooModelBadLayoutType {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[dojo::model]
pub struct FooModelMemberRemoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
}

#[dojo::model]
pub struct FooModelMemberAddedButRemoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
    pub c: u256,
    pub d: u256,
}

#[dojo::model]
pub struct FooModelMemberAddedButMoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
    pub a: felt252,
    pub c: u256,
}

#[dojo::model]
pub struct FooModelMemberAdded {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
    pub c: u256,
}

#[dojo::model]
struct FooModelMemberChanged {
    #[key]
    pub caller: ContractAddress,
    pub a: (MyEnum, u8, u32),
    pub b: u128,
}

#[dojo::model]
struct FooModelMemberIllegalChange {
    #[key]
    pub caller: ContractAddress,
    pub a: MyEnum,
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
pub struct ModelWithSignedInt {
    #[key]
    pub caller: ContractAddress,
    pub a: i8,
    pub b: i16,
    pub c: i32,
    pub d: i64,
    pub e: i128,
}

#[test]
fn test_register_model_for_namespace_owner() {
    let bob: ContractAddress = 0xb0b.try_into().unwrap();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);

    let mut spy = spy_events();

    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    let class_hash = dojo_snf_test::declare_model_contract("Foo");
    world.register_model("dojo", class_hash);

    // parse the event manually because we don't know the value of
    // the 'address' field of the emitted event to assert a full event.
    let events = spy.get_events().emitted_by(world.contract_address);

    assert(events.events.len() == 1, 'There should be one event');

    let (_, event) = events.events.at(0);
    let mut keys = event.keys.span();

    let event_name = *keys.pop_front().unwrap();
    let name: ByteArray = core::serde::Serde::deserialize(ref keys).unwrap();
    let ns: ByteArray = core::serde::Serde::deserialize(ref keys).unwrap();

    assert(event_name == selector!("ModelRegistered"), 'Wrong event name');
    assert(name == "Foo", 'Wrong name');
    assert(ns == "dojo", 'Wrong namespace');
    assert(event.data.at(0) == @class_hash.into(), 'Wrong class hash');

    assert(world.is_owner(Model::<Foo>::selector(DOJO_NSH), bob), 'bob is not the owner');
}


#[test]
#[should_panic(
    expected: "Name `foo-bis` is invalid according to Dojo naming rules: ^[a-zA-Z0-9_]+$",
)]
fn test_register_model_with_invalid_name() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_model("dojo", dojo_snf_test::declare_model_contract("FooInvalidName"));
}

#[test]
#[should_panic(expected: "Account `0xb0b` does NOT have OWNER role on namespace `dojo`")]
fn test_register_model_for_namespace_writer() {
    let bob: ContractAddress = 0xb0b.try_into().unwrap();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_writer(DOJO_NSH, bob);

    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    world.register_model("dojo", dojo_snf_test::declare_model_contract("Foo"));
}

#[test]
fn test_upgrade_model_from_model_owner() {
    let bob: ContractAddress = 0xb0b.try_into().unwrap();

    let world = deploy_world_for_model_upgrades();
    world.grant_owner(Model::<FooModelMemberAdded>::selector(DOJO_NSH), bob);

    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    let mut spy = spy_events();

    let class_hash = dojo_snf_test::declare_model_contract("FooModelMemberAdded");
    world.upgrade_model("dojo", class_hash);

    // parse the event manually because we don't know the value of
    // the 'address' field of the emitted event to assert a full event.
    let events = spy.get_events().emitted_by(world.contract_address);

    assert(events.events.len() == 1, 'There should be one event');

    let (_, event) = events.events.at(0);

    assert(event.keys.at(0) == @selector!("ModelUpgraded"), 'Wrong event name');
    assert(event.keys.at(1) == @Model::<FooModelMemberAdded>::selector(DOJO_NSH), 'Wrong selector');
    assert(event.data.at(0) == @class_hash.into(), 'Wrong class hash');

    assert(
        world.is_owner(Model::<FooModelMemberAdded>::selector(DOJO_NSH), bob),
        'bob is not the owner',
    );
}

#[test]
fn test_upgrade_model() {
    let caller: ContractAddress = 0xb0b.try_into().unwrap();

    let world = deploy_world_for_model_upgrades();
    let mut world_storage = dojo::world::WorldStorageTrait::new(world, @"dojo");

    let mut spy = spy_events();

    let class_hash = dojo_snf_test::declare_model_contract("FooModelMemberAdded");
    world.upgrade_model("dojo", class_hash);

    // parse the event manually because we don't know the value of
    // the 'address' field of the emitted event to assert a full event.
    let events = spy.get_events().emitted_by(world.contract_address);

    assert(events.events.len() == 1, 'There should be one event');

    let (_, event) = events.events.at(0);

    assert(event.keys.at(0) == @selector!("ModelUpgraded"), 'Wrong event name');
    assert(event.keys.at(1) == @Model::<FooModelMemberAdded>::selector(DOJO_NSH), 'Wrong selector');
    assert(event.data.at(0) == @class_hash.into(), 'Wrong class hash');

    // values previously set in deploy_world_for_model_upgrades
    let read: FooModelMemberAdded = world_storage.read_model(caller);
    assert!(read.a == 123);
    assert!(read.b == 456);
    assert!(read.c == 0);
}

#[test]
fn test_upgrade_model_with_member_changed() {
    let caller: ContractAddress = 0xb0b.try_into().unwrap();
    let world = deploy_world_for_model_upgrades();
    let mut world_storage = dojo::world::WorldStorageTrait::new(world, @"dojo");

    let mut spy = spy_events();

    let class_hash = dojo_snf_test::declare_model_contract("FooModelMemberChanged");
    world.upgrade_model("dojo", class_hash);

    // parse the event manually because we don't know the value of
    // the 'address' field of the emitted event to assert a full event.
    let events = spy.get_events().emitted_by(world.contract_address);

    assert(events.events.len() == 1, 'There should be one event');

    let (_, event) = events.events.at(0);

    assert(event.keys.at(0) == @selector!("ModelUpgraded"), 'Wrong event name');
    assert(
        event.keys.at(1) == @Model::<FooModelMemberChanged>::selector(DOJO_NSH), 'Wrong selector',
    );
    assert(event.data.at(0) == @class_hash.into(), 'Wrong class hash');

    // values previously set in deploy_world_for_model_upgrades
    let read: FooModelMemberChanged = world_storage.read_model(caller);
    assert!(read.a == (MyEnum::X(42), 189, 0));
    assert!(read.b == 456);
}

#[test]
#[should_panic(expected: "Invalid new layout to upgrade the resource `dojo-FooModelBadLayoutType`")]
fn test_upgrade_model_with_bad_layout_type() {
    let world = deploy_world_for_model_upgrades();
    world.upgrade_model("dojo", dojo_snf_test::declare_model_contract("FooModelBadLayoutType"));
}

#[test]
#[should_panic(expected: "Invalid new schema to upgrade the resource `dojo-FooModelMemberRemoved`")]
fn test_upgrade_model_with_member_removed() {
    let world = deploy_world_for_model_upgrades();
    world.upgrade_model("dojo", dojo_snf_test::declare_model_contract("FooModelMemberRemoved"));
}

#[test]
#[should_panic(
    expected: "Invalid new schema to upgrade the resource `dojo-FooModelMemberAddedButRemoved`",
)]
fn test_upgrade_model_with_member_added_but_removed() {
    let world = deploy_world_for_model_upgrades();
    world
        .upgrade_model(
            "dojo", dojo_snf_test::declare_model_contract("FooModelMemberAddedButRemoved"),
        );
}

#[test]
#[should_panic(
    expected: "Invalid new schema to upgrade the resource `dojo-FooModelMemberAddedButMoved`",
)]
fn test_upgrade_model_with_member_moved() {
    let world = deploy_world_for_model_upgrades();
    world
        .upgrade_model(
            "dojo", dojo_snf_test::declare_model_contract("FooModelMemberAddedButMoved"),
        );
}

#[test]
#[should_panic(
    expected: "Invalid new schema to upgrade the resource `dojo-FooModelMemberIllegalChange`",
)]
fn test_upgrade_model_with_member_illegal_change() {
    let world = deploy_world_for_model_upgrades();
    world
        .upgrade_model(
            "dojo", dojo_snf_test::declare_model_contract("FooModelMemberIllegalChange"),
        );
}

#[test]
#[should_panic(
    expected: "Account `0xa11ce` does NOT have OWNER role on model (or its namespace) `FooModelMemberAdded`",
)]
fn test_upgrade_model_from_model_writer() {
    let alice: ContractAddress = 0xa11ce.try_into().unwrap();

    let world = deploy_world_for_model_upgrades();

    world.grant_writer(Model::<FooModelMemberAdded>::selector(DOJO_NSH), alice);

    dojo_snf_test::set_account_address(alice);
    dojo_snf_test::set_caller_address(alice);

    world.upgrade_model("dojo", dojo_snf_test::declare_model_contract("FooModelMemberAdded"));
}

#[test]
#[should_panic(expected: "Resource (Model) `dojo-Foo` is already registered")]
fn test_upgrade_model_from_random_account() {
    let bob: ContractAddress = 0xb0b.try_into().unwrap();
    let alice: ContractAddress = 0xa11ce.try_into().unwrap();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);
    world.grant_owner(DOJO_NSH, alice);

    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);
    world.register_model("dojo", dojo_snf_test::declare_model_contract("Foo"));

    dojo_snf_test::set_account_address(alice);
    dojo_snf_test::set_caller_address(alice);
    world.register_model("dojo", dojo_snf_test::declare_model_contract("Foo"));
}

#[test]
#[should_panic(expected: "Namespace `another_namespace` is not registered")]
fn test_register_model_with_unregistered_namespace() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_model("another_namespace", dojo_snf_test::declare_model_contract("Foo"));
}

// It's ENTRYPOINT_NOT_FOUND for now as in this example the contract is not a dojo contract
// and it's not the account that is calling the register_model function.
#[test]
#[should_panic(
    expected: "Contract `0x1a5ee9565583195ae8bf4038b95cef58db4f6e78089dd80ce5da3a2518ba7a1` does NOT have OWNER role on namespace `dojo`",
)]
fn test_register_model_through_malicious_contract() {
    let bob: ContractAddress = 0xb0b.try_into().unwrap();
    let malicious_contract = dojo_snf_test::declare_and_deploy("malicious_contract");

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);

    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(malicious_contract);
    world.register_model("dojo", dojo_snf_test::declare_model_contract("Foo"));
}

#[test]
fn test_write_read_model_with_signed_int() {
    let mut world = deploy_world();
    let world_d = world.dispatcher;

    world_d.register_model("dojo", dojo_snf_test::declare_model_contract("ModelWithSignedInt"));

    let addr = starknet::get_contract_address();

    let mut model = ModelWithSignedInt { caller: addr, a: -1, b: -2, c: -3, d: -4, e: -5 };

    world.write_model(@model);

    let read: ModelWithSignedInt = world.read_model(addr);

    assert!(read.a == model.a);
    assert!(read.b == model.b);
    assert!(read.c == model.c);
    assert!(read.d == model.d);
    assert!(read.e == model.e);
}

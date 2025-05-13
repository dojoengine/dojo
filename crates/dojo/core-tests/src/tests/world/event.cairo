use dojo::event::Event;
use dojo::world::{IWorldDispatcherTrait, world};
use dojo_snf_test;
use snforge_std::{EventSpyTrait, EventsFilterTrait, spy_events};
use starknet::ContractAddress;
use crate::tests::helpers::{
    DOJO_NSH, MyEnum, SimpleEvent, deploy_world, deploy_world_for_event_upgrades,
};

#[dojo::event]
pub struct FooEventBadLayoutType {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[dojo::event]
pub struct FooEventMemberRemoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
}

#[dojo::event]
pub struct FooEventMemberAddedButRemoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
    pub c: u256,
    pub d: u256,
}

#[dojo::event]
pub struct FooEventMemberAddedButMoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
    pub a: felt252,
    pub c: u256,
}

#[dojo::event]
pub struct FooEventMemberAdded {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
    pub c: u256,
}

#[dojo::event]
struct FooEventMemberChanged {
    #[key]
    pub caller: ContractAddress,
    pub a: (MyEnum, u8, u32),
    pub b: u128,
}

#[dojo::event]
struct FooEventMemberIllegalChange {
    #[key]
    pub caller: ContractAddress,
    pub a: MyEnum,
    pub b: u128,
}

#[test]
fn test_register_event_for_namespace_owner() {
    let bob: ContractAddress = 0xb0b.try_into().unwrap();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);

    let mut spy = spy_events();

    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    let class_hash = dojo_snf_test::declare_event_contract("SimpleEvent");
    world.register_event("dojo", class_hash);

    // parse the event manually because we don't know the value of
    // the 'address' field of the emitted event to assert a full event.
    let events = spy.get_events().emitted_by(world.contract_address);

    assert(events.events.len() == 1, 'There should be one event');

    let (_, event) = events.events.at(0);
    let mut keys = event.keys.span();

    let event_name = *keys.pop_front().unwrap();
    let name: ByteArray = core::serde::Serde::deserialize(ref keys).unwrap();
    let ns: ByteArray = core::serde::Serde::deserialize(ref keys).unwrap();

    assert(event_name == selector!("EventRegistered"), 'Wrong event name');
    assert(name == "SimpleEvent", 'Wrong name');
    assert(ns == "dojo", 'Wrong namespace');
    assert(event.data.at(0) == @class_hash.into(), 'Wrong class hash');

    assert(world.is_owner(Event::<SimpleEvent>::selector(DOJO_NSH), bob), 'bob is not the owner');
}

#[test]
#[should_panic(expected: "Account `2827` does NOT have OWNER role on namespace `dojo`")]
fn test_register_event_for_namespace_writer() {
    let bob: ContractAddress = 0xb0b.try_into().unwrap();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_writer(DOJO_NSH, bob);

    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    world.register_event("dojo", dojo_snf_test::declare_event_contract("SimpleEvent"));
}

#[test]
fn test_upgrade_event_from_event_owner() {
    let bob: ContractAddress = 0xb0b.try_into().unwrap();

    let world = deploy_world_for_event_upgrades();
    world.grant_owner(Event::<FooEventMemberAdded>::selector(DOJO_NSH), bob);

    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    let mut spy = spy_events();

    let class_hash = dojo_snf_test::declare_event_contract("FooEventMemberAdded");
    world.upgrade_event("dojo", class_hash);

    // parse the event manually because we don't know the value of
    // the 'address' field of the emitted event to assert a full event.
    let events = spy.get_events().emitted_by(world.contract_address);

    assert(events.events.len() == 1, 'There should be one event');

    let (_, event) = events.events.at(0);

    assert(event.keys.at(0) == @selector!("EventUpgraded"), 'Wrong event name');
    assert(
        event.keys.at(1) == @Event::<FooEventMemberAdded>::selector(DOJO_NSH), 'bad model selector',
    );
    assert(event.data.at(0) == @class_hash.into(), 'Wrong class hash');

    assert(
        world.is_owner(Event::<FooEventMemberAdded>::selector(DOJO_NSH), bob),
        'bob is not the owner',
    );
}

#[test]
fn test_upgrade_event() {
    let world = deploy_world_for_event_upgrades();

    let mut spy = spy_events();

    let class_hash = dojo_snf_test::declare_event_contract("FooEventMemberAdded");
    world.upgrade_event("dojo", class_hash);

    // parse the event manually because we don't know the value of
    // the 'address' field of the emitted event to assert a full event.
    let events = spy.get_events().emitted_by(world.contract_address);

    assert(events.events.len() == 1, 'There should be one event');

    let (_, event) = events.events.at(0);

    assert(event.keys.at(0) == @selector!("EventUpgraded"), 'Wrong event name');
    assert(
        event.keys.at(1) == @Event::<FooEventMemberAdded>::selector(DOJO_NSH), 'bad model selector',
    );
    assert(event.data.at(0) == @class_hash.into(), 'Wrong class hash');
}

#[test]
fn test_upgrade_event_with_member_changed() {
    let world = deploy_world_for_event_upgrades();

    let mut spy = spy_events();

    let class_hash = dojo_snf_test::declare_event_contract("FooEventMemberChanged");
    world.upgrade_event("dojo", class_hash);

    // parse the event manually because we don't know the value of
    // the 'address' field of the emitted event to assert a full event.
    let events = spy.get_events().emitted_by(world.contract_address);

    assert(events.events.len() == 1, 'There should be one event');

    let (_, event) = events.events.at(0);

    assert(event.keys.at(0) == @selector!("EventUpgraded"), 'Wrong event name');
    assert(
        event.keys.at(1) == @Event::<FooEventMemberChanged>::selector(DOJO_NSH),
        'bad model selector',
    );
    assert(event.data.at(0) == @class_hash.into(), 'Wrong class hash');
}

#[test]
#[should_panic(expected: "Invalid new layout to upgrade the resource `dojo-FooEventBadLayoutType`")]
fn test_upgrade_event_with_bad_layout_type() {
    let world = deploy_world_for_event_upgrades();
    world.upgrade_event("dojo", dojo_snf_test::declare_event_contract("FooEventBadLayoutType"));
}

#[test]
#[should_panic(expected: "Invalid new schema to upgrade the resource `dojo-FooEventMemberRemoved`")]
fn test_upgrade_event_with_member_removed() {
    let world = deploy_world_for_event_upgrades();
    world.upgrade_event("dojo", dojo_snf_test::declare_event_contract("FooEventMemberRemoved"));
}

#[test]
#[should_panic(
    expected: "Invalid new schema to upgrade the resource `dojo-FooEventMemberAddedButRemoved`",
)]
fn test_upgrade_event_with_member_added_but_removed() {
    let world = deploy_world_for_event_upgrades();
    world
        .upgrade_event(
            "dojo", dojo_snf_test::declare_event_contract("FooEventMemberAddedButRemoved"),
        );
}

#[test]
#[should_panic(
    expected: "Invalid new schema to upgrade the resource `dojo-FooEventMemberAddedButMoved`",
)]
fn test_upgrade_event_with_member_moved() {
    let world = deploy_world_for_event_upgrades();
    world
        .upgrade_event(
            "dojo", dojo_snf_test::declare_event_contract("FooEventMemberAddedButMoved"),
        );
}

#[test]
#[should_panic(
    expected: "Invalid new schema to upgrade the resource `dojo-FooEventMemberIllegalChange`",
)]
fn test_upgrade_event_with_member_illegal_change() {
    let world = deploy_world_for_event_upgrades();
    world
        .upgrade_event(
            "dojo", dojo_snf_test::declare_event_contract("FooEventMemberIllegalChange"),
        );
}

#[test]
#[should_panic(
    expected: "Account `659918` does NOT have OWNER role on event (or its namespace) `FooEventMemberAdded`",
)]
fn test_upgrade_event_from_event_writer() {
    let alice: ContractAddress = 0xa11ce.try_into().unwrap();

    let world = deploy_world_for_event_upgrades();

    world.grant_writer(Event::<FooEventMemberAdded>::selector(DOJO_NSH), alice);

    dojo_snf_test::set_account_address(alice);
    dojo_snf_test::set_caller_address(alice);

    world.upgrade_event("dojo", dojo_snf_test::declare_event_contract("FooEventMemberAdded"));
}

#[test]
#[should_panic(expected: "Resource (Event) `dojo-SimpleEvent` is already registered")]
fn test_upgrade_event_from_random_account() {
    let bob: ContractAddress = 0xb0b.try_into().unwrap();
    let alice: ContractAddress = 0xa11ce.try_into().unwrap();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);
    world.grant_owner(DOJO_NSH, alice);

    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(bob);

    world.register_event("dojo", dojo_snf_test::declare_event_contract("SimpleEvent"));

    dojo_snf_test::set_account_address(alice);
    dojo_snf_test::set_caller_address(alice);

    world.register_event("dojo", dojo_snf_test::declare_event_contract("SimpleEvent"));
}

#[test]
#[should_panic(expected: "Namespace `another_namespace` is not registered")]
fn test_register_event_with_unregistered_namespace() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_event("another_namespace", dojo_snf_test::declare_event_contract("SimpleEvent"));
}

// It's ENTRYPOINT_NOT_FOUND for now as in this example the contract is not a dojo contract
// and it's not the account that is calling the register_event function.
#[test]
#[should_panic(expected: ('ENTRYPOINT_NOT_FOUND', 'ENTRYPOINT_FAILED'))]
fn test_register_event_through_malicious_contract() {
    let bob: ContractAddress = 0xb0b.try_into().unwrap();
    let malicious_contract = dojo_snf_test::declare_and_deploy("malicious_contract");

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);

    dojo_snf_test::set_account_address(bob);
    dojo_snf_test::set_caller_address(malicious_contract);

    world.register_event("dojo", dojo_snf_test::declare_event_contract("SimpleEvent"));
}

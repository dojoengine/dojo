use dojo::model::ModelStorage;
use core::starknet::ContractAddress;

use crate::tests::helpers::{
    Foo, m_Foo, DOJO_NSH, drop_all_events, deploy_world, deploy_world_for_model_upgrades,
    foo_invalid_name,
};
use dojo::world::{world, IWorldDispatcherTrait};
use dojo::model::Model;


#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
pub struct FooModelBadLayoutType {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
pub struct FooModelMemberRemoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
pub struct FooModelMemberAddedButRemoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
    pub c: u256,
    pub d: u256,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
pub struct FooModelMemberAddedButMoved {
    #[key]
    pub caller: ContractAddress,
    pub b: u128,
    pub a: felt252,
    pub c: u256,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
pub struct FooModelMemberAdded {
    #[key]
    pub caller: ContractAddress,
    pub a: felt252,
    pub b: u128,
    pub c: u256,
}

#[derive(Introspect, Copy, Drop, Serde, PartialEq)]
enum MyEnum {
    X: u8,
    Y: u16,
}

#[derive(Introspect, Copy, Drop, Serde)]
#[dojo::model]
struct FooModelMemberChanged {
    #[key]
    pub caller: ContractAddress,
    pub a: (MyEnum, u8, u32),
    pub b: u128,
}

#[derive(Introspect, Copy, Drop, Serde)]
enum AnotherEnum {
    X: bool,
}

#[derive(Introspect, Copy, Drop, Serde)]
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
    let bob = starknet::contract_address_const::<0xb0b>();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);

    drop_all_events(world.contract_address);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.register_model("dojo", m_Foo::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::ModelRegistered(event) = event.unwrap() {
        assert(event.name == Model::<Foo>::name(), 'bad event name');
        assert(event.namespace == "dojo", 'bad event namespace');
        assert(
            event.class_hash == m_Foo::TEST_CLASS_HASH.try_into().unwrap(), 'bad event class_hash',
        );
        assert(
            event.address != core::num::traits::Zero::<ContractAddress>::zero(),
            'bad event prev address',
        );
    } else {
        core::panic_with_felt252('no ModelRegistered event');
    }

    assert(world.is_owner(Model::<Foo>::selector(DOJO_NSH), bob), 'bob is not the owner');
}

#[test]
#[should_panic(
    expected: (
        "Name `foo-bis` is invalid according to Dojo naming rules: ^[a-zA-Z0-9_]+$",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_register_model_with_invalid_name() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_model("dojo", foo_invalid_name::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: ("Account `0xb0b` does NOT have OWNER role on namespace `dojo`", 'ENTRYPOINT_FAILED'),
)]
fn test_register_model_for_namespace_writer() {
    let bob = starknet::contract_address_const::<0xb0b>();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_writer(DOJO_NSH, bob);

    drop_all_events(world.contract_address);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.register_model("dojo", m_Foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
fn test_upgrade_model_from_model_owner() {
    let bob = starknet::contract_address_const::<0xb0b>();

    let world = deploy_world_for_model_upgrades();
    world.grant_owner(Model::<FooModelMemberAdded>::selector(DOJO_NSH), bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    drop_all_events(world.contract_address);

    world.upgrade_model("dojo", m_FooModelMemberAdded::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::ModelUpgraded(event) = event.unwrap() {
        assert(
            event.selector == Model::<FooModelMemberAdded>::selector(DOJO_NSH),
            'bad model selector',
        );
        assert(
            event.class_hash == m_FooModelMemberAdded::TEST_CLASS_HASH.try_into().unwrap(),
            'bad model class_hash',
        );
        assert(
            event.address != core::num::traits::Zero::<ContractAddress>::zero(),
            'bad model prev address',
        );
    } else {
        core::panic_with_felt252('no ModelUpgraded event');
    }

    assert(
        world.is_owner(Model::<FooModelMemberAdded>::selector(DOJO_NSH), bob),
        'bob is not the owner',
    );
}

#[test]
fn test_upgrade_model() {
    let caller = starknet::contract_address_const::<0xb0b>();

    let world = deploy_world_for_model_upgrades();
    let mut world_storage = dojo::world::WorldStorageTrait::new(world, @"dojo");

    drop_all_events(world.contract_address);

    world.upgrade_model("dojo", m_FooModelMemberAdded::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::ModelUpgraded(event) = event.unwrap() {
        assert(
            event.selector == Model::<FooModelMemberAdded>::selector(DOJO_NSH),
            'bad model selector',
        );
        assert(
            event.class_hash == m_FooModelMemberAdded::TEST_CLASS_HASH.try_into().unwrap(),
            'bad model class_hash',
        );
        assert(
            event.address != core::num::traits::Zero::<ContractAddress>::zero(),
            'bad model address',
        );
    } else {
        core::panic_with_felt252('no ModelUpgraded event');
    }

    // values previously set in deploy_world_for_model_upgrades
    let read: FooModelMemberAdded = world_storage.read_model(caller);
    assert!(read.a == 123);
    assert!(read.b == 456);
    assert!(read.c == 0);
}

#[test]
fn test_upgrade_model_with_member_changed() {
    let caller = starknet::contract_address_const::<0xb0b>();
    let world = deploy_world_for_model_upgrades();
    let mut world_storage = dojo::world::WorldStorageTrait::new(world, @"dojo");

    drop_all_events(world.contract_address);

    world.upgrade_model("dojo", m_FooModelMemberChanged::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::ModelUpgraded(event) = event.unwrap() {
        assert(
            event.selector == Model::<FooModelMemberChanged>::selector(DOJO_NSH),
            'bad model selector',
        );
        assert(
            event.class_hash == m_FooModelMemberChanged::TEST_CLASS_HASH.try_into().unwrap(),
            'bad model class_hash',
        );
        assert(
            event.address != core::num::traits::Zero::<ContractAddress>::zero(),
            'bad model address',
        );
    } else {
        core::panic_with_felt252('no ModelUpgraded event');
    }

    // values previously set in deploy_world_for_model_upgrades
    let read: FooModelMemberChanged = world_storage.read_model(caller);
    assert!(read.a == (MyEnum::X(42), 189, 0));
    assert!(read.b == 456);
}

#[test]
#[should_panic(
    expected: (
        "Invalid new layout to upgrade the resource `dojo-FooModelBadLayoutType`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_model_with_bad_layout_type() {
    let world = deploy_world_for_model_upgrades();
    world.upgrade_model("dojo", m_FooModelBadLayoutType::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: (
        "Invalid new schema to upgrade the resource `dojo-FooModelMemberRemoved`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_model_with_member_removed() {
    let world = deploy_world_for_model_upgrades();
    world.upgrade_model("dojo", m_FooModelMemberRemoved::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: (
        "Invalid new schema to upgrade the resource `dojo-FooModelMemberAddedButRemoved`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_model_with_member_added_but_removed() {
    let world = deploy_world_for_model_upgrades();
    world
        .upgrade_model(
            "dojo", m_FooModelMemberAddedButRemoved::TEST_CLASS_HASH.try_into().unwrap(),
        );
}

#[test]
#[should_panic(
    expected: (
        "Invalid new schema to upgrade the resource `dojo-FooModelMemberAddedButMoved`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_model_with_member_moved() {
    let world = deploy_world_for_model_upgrades();
    world.upgrade_model("dojo", m_FooModelMemberAddedButMoved::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: (
        "Invalid new schema to upgrade the resource `dojo-FooModelMemberIllegalChange`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_model_with_member_illegal_change() {
    let world = deploy_world_for_model_upgrades();
    world.upgrade_model("dojo", m_FooModelMemberIllegalChange::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: (
        "Account `0xa11ce` does NOT have OWNER role on model (or its namespace) `FooModelMemberAdded`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_upgrade_model_from_model_writer() {
    let alice = starknet::contract_address_const::<0xa11ce>();

    let world = deploy_world_for_model_upgrades();

    world.grant_writer(Model::<FooModelMemberAdded>::selector(DOJO_NSH), alice);

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);
    world.upgrade_model("dojo", m_FooModelMemberAdded::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: ("Resource (Model) `dojo-Foo` is already registered", 'ENTRYPOINT_FAILED'),
)]
fn test_upgrade_model_from_random_account() {
    let bob = starknet::contract_address_const::<0xb0b>();
    let alice = starknet::contract_address_const::<0xa11ce>();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);
    world.grant_owner(DOJO_NSH, alice);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.register_model("dojo", m_Foo::TEST_CLASS_HASH.try_into().unwrap());

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);
    world.register_model("dojo", m_Foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(expected: ("Namespace `another_namespace` is not registered", 'ENTRYPOINT_FAILED'))]
fn test_register_model_with_unregistered_namespace() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_model("another_namespace", m_Foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: (
        "Contract `0xdead` does NOT have OWNER role on namespace `dojo`", 'ENTRYPOINT_FAILED',
    ),
)]
fn test_register_model_through_malicious_contract() {
    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    let world = deploy_world();
    let world = world.dispatcher;

    world.grant_owner(DOJO_NSH, bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(malicious_contract);
    world.register_model("dojo", m_Foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
fn test_write_read_model_with_signed_int() {
    let mut world = deploy_world();
    let world_d = world.dispatcher;

    world_d.register_model("dojo", m_ModelWithSignedInt::TEST_CLASS_HASH.try_into().unwrap());

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

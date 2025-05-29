use dojo::world::Resource;
use dojo::world::world::Event as WorldEvent;
use dojo::utils::bytearray_hash;
use dojo::world::{
    IWorldDispatcher, IWorldDispatcherTrait, IUpgradeableWorldDispatcher,
    IUpgradeableWorldDispatcherTrait, WorldStorageTrait,
};
use dojo::model::ModelStorage;
use dojo::event::{Event, EventStorage};

use crate::tests::helpers::{
    bar, IbarDispatcherTrait, drop_all_events, deploy_world_and_bar, Foo, m_Foo, test_contract,
    test_contract_with_dojo_init_args, SimpleEvent, e_SimpleEvent, deploy_world, library_a,
    LibraryALibraryDispatcher, LibraryADispatcherTrait,
};
use crate::{spawn_test_world, ContractDefTrait, NamespaceDef, TestResource, WorldStorageTestTrait};

#[test]
#[available_gas(20000000)]
fn test_model() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_model("dojo", m_Foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
fn test_system() {
    let (world, bar_contract) = deploy_world_and_bar();

    bar_contract.set_foo(1337, 1337);

    let stored: Foo = world.read_model(starknet::get_caller_address());
    assert(stored.a == 1337, 'data not stored');
    assert(stored.b == 1337, 'data not stored');
}

#[test]
fn test_delete() {
    let (world, bar_contract) = deploy_world_and_bar();

    bar_contract.set_foo(1337, 1337);
    let stored: Foo = world.read_model(starknet::get_caller_address());
    assert(stored.a == 1337, 'data not stored');
    assert(stored.b == 1337, 'data not stored');

    bar_contract.delete_foo();

    let deleted: Foo = world.read_model(starknet::get_caller_address());
    assert(deleted.a == 0, 'data not deleted');
    assert(deleted.b == 0, 'data not deleted');
}

#[test]
#[available_gas(6000000)]
fn test_contract_getter() {
    let world = deploy_world();
    let world = world.dispatcher;

    let address = world
        .register_contract('salt1', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());

    if let Resource::Contract((contract_address, namespace_hash)) = world
        .resource(selector_from_tag!("dojo-test_contract")) {
        assert(address == contract_address, 'invalid contract address');

        assert(namespace_hash == bytearray_hash(@"dojo"), 'invalid namespace hash');
    }
}

#[test]
#[available_gas(6000000)]
fn test_emit() {
    let bob = starknet::contract_address_const::<0xb0b>();

    let namespace_def = NamespaceDef {
        namespace: "dojo", resources: [TestResource::Event(e_SimpleEvent::TEST_CLASS_HASH),].span(),
    };

    let mut world = spawn_test_world([namespace_def].span());

    let bob_def = ContractDefTrait::new_address(bob)
        .with_writer_of([world.resource_selector(@"SimpleEvent")].span());
    world.sync_perms_and_inits([bob_def].span());

    drop_all_events(world.dispatcher.contract_address);

    starknet::testing::set_contract_address(bob);

    let simple_event = SimpleEvent { id: 2, data: (3, 4) };
    world.emit_event(@simple_event);

    let event = starknet::testing::pop_log::<WorldEvent>(world.dispatcher.contract_address);

    assert(event.is_some(), 'no event');

    if let WorldEvent::EventEmitted(event) = event.unwrap() {
        assert(
            event.selector == Event::<SimpleEvent>::selector(world.namespace_hash),
            'bad event selector',
        );
        assert(event.system_address == bob, 'bad system address');
        assert(event.keys == [2].span(), 'bad keys');
        assert(event.values == [3, 4].span(), 'bad values');
    } else {
        core::panic_with_felt252('no EventEmitted event');
    }
}

#[test]
fn test_execute_multiple_worlds() {
    let (world1, bar1_contract) = deploy_world_and_bar();
    let (world2, bar2_contract) = deploy_world_and_bar();

    let alice = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(alice);

    bar1_contract.set_foo(1337, 1337);
    bar2_contract.set_foo(7331, 7331);

    let data1: Foo = world1.read_model(alice);
    let data2: Foo = world2.read_model(alice);

    assert(data1.a == 1337, 'data1 not stored');
    assert(data2.a == 7331, 'data2 not stored');
}

#[starknet::interface]
trait IWorldUpgrade<TContractState> {
    fn hello(self: @TContractState) -> felt252;
}

#[starknet::contract]
mod worldupgrade {
    use super::IWorldDispatcher;

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
    let world = deploy_world();
    let world = world.dispatcher;

    let mut upgradeable_world_dispatcher = IUpgradeableWorldDispatcher {
        contract_address: world.contract_address,
    };
    upgradeable_world_dispatcher.upgrade(worldupgrade::TEST_CLASS_HASH.try_into().unwrap());

    let res = (IWorldUpgradeDispatcher { contract_address: world.contract_address }).hello();

    assert(res == 'dojo', 'should return dojo');
}

#[test]
#[available_gas(60000000)]
#[should_panic(expected: ('invalid class_hash', 'ENTRYPOINT_FAILED'))]
fn test_upgradeable_world_with_class_hash_zero() {
    let world = deploy_world();
    let world = world.dispatcher;

    let admin = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_account_contract_address(admin);
    starknet::testing::set_contract_address(admin);

    let mut upgradeable_world_dispatcher = IUpgradeableWorldDispatcher {
        contract_address: world.contract_address,
    };
    upgradeable_world_dispatcher.upgrade(0.try_into().unwrap());
}

#[test]
#[available_gas(60000000)]
#[should_panic(
    expected: ("Caller `4919` cannot upgrade the resource `0` (not owner)", 'ENTRYPOINT_FAILED'),
)]
fn test_upgradeable_world_from_non_owner() {
    // Deploy world contract
    let world = deploy_world();
    let world = world.dispatcher;

    let not_owner = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(not_owner);
    starknet::testing::set_account_contract_address(not_owner);

    let mut upgradeable_world_dispatcher = IUpgradeableWorldDispatcher {
        contract_address: world.contract_address,
    };
    upgradeable_world_dispatcher.upgrade(worldupgrade::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[available_gas(6000000)]
fn test_constructor_default() {
    let world = deploy_world();
    let world = world.dispatcher;

    let _address = world
        .register_contract('salt1', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
fn test_can_call_init_only_world() {
    let world = deploy_world();
    let world = world.dispatcher;

    let address = world
        .register_contract('salt1', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());

    let expected_panic: ByteArray =
        "Only the world can init contract `test_contract`, but caller is `0`";

    match starknet::syscalls::call_contract_syscall(
        address, dojo::world::world::DOJO_INIT_SELECTOR, [].span(),
    ) {
        Result::Ok(_) => panic!("should panic"),
        Result::Err(e) => {
            let mut s = e.span();
            // Remove the out of range error.
            s.pop_front().unwrap();
            // Remove the ENTRYPOINT_FAILED suffix.
            s.pop_back().unwrap();

            let e_str: ByteArray = Serde::deserialize(ref s).expect('failed deser');
            println!("e_str: {}", e_str);
            assert_eq!(e_str, expected_panic);
        },
    }
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED', 'ENTRYPOINT_FAILED'))]
fn test_can_call_init_only_owner() {
    let world = deploy_world();
    let world = world.dispatcher;

    let _address = world
        .register_contract('salt1', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());

    let bob = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(bob);

    world.init_contract(selector_from_tag!("dojo-test_contract"), [].span());
}

#[test]
#[available_gas(6000000)]
fn test_can_call_init_default() {
    let world = deploy_world();
    let world = world.dispatcher;

    let _address = world
        .register_contract('salt1', "dojo", test_contract::TEST_CLASS_HASH.try_into().unwrap());

    world.init_contract(selector_from_tag!("dojo-test_contract"), [].span());
}

#[test]
#[available_gas(6000000)]
fn test_can_call_init_args() {
    let world = deploy_world();
    let world = world.dispatcher;

    let _address = world
        .register_contract(
            'salt1', "dojo", test_contract_with_dojo_init_args::TEST_CLASS_HASH.try_into().unwrap(),
        );

    world.init_contract(selector_from_tag!("dojo-test_contract_with_dojo_init_args"), [1].span());
}

#[test]
fn test_can_call_init_only_world_args() {
    let world = deploy_world();
    let world = world.dispatcher;

    let address = world
        .register_contract(
            'salt1', "dojo", test_contract_with_dojo_init_args::TEST_CLASS_HASH.try_into().unwrap(),
        );

    let expected_panic: ByteArray =
        "Only the world can init contract `test_contract_with_dojo_init_args`, but caller is `0`";

    match starknet::syscalls::call_contract_syscall(
        address, dojo::world::world::DOJO_INIT_SELECTOR, [123].span(),
    ) {
        Result::Ok(_) => panic!("should panic"),
        Result::Err(e) => {
            let mut s = e.span();
            // Remove the out of range error.
            s.pop_front().unwrap();
            // Remove the ENTRYPOINT_FAILED suffix.
            s.pop_back().unwrap();

            let e_str: ByteArray = Serde::deserialize(ref s).expect('failed deser');

            assert_eq!(e_str, expected_panic);
        },
    }
}

#[test]
pub fn dns_valid_class_hash() {
    let namespace_def = NamespaceDef {
        namespace: "dojo",
        resources: [
            TestResource::Model(m_Foo::TEST_CLASS_HASH),
            TestResource::Contract(bar::TEST_CLASS_HASH),
        ]
            .span(),
    };

    let mut world = spawn_test_world([namespace_def].span());
    world.sync_perms_and_inits([].span());

    let (_, class_hash) = world.dns(@"bar").unwrap();
    assert(class_hash == bar::TEST_CLASS_HASH.try_into().unwrap(), 'should return bar class hash');
}

#[test]
fn test_register_library() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_library("dojo", library_a::TEST_CLASS_HASH.try_into().unwrap(), "liba", "0_1_0");
}

#[test]
#[should_panic(
    expected: (
        "Resource (Library) `dojo-liba_v0_1_0` is already registered. Libraries can't be updated, increment the version in the Dojo configuration file instead.",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_register_library_already_registered() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_library("dojo", library_a::TEST_CLASS_HASH.try_into().unwrap(), "liba", "0_1_0");

    world.register_library("dojo", library_a::TEST_CLASS_HASH.try_into().unwrap(), "liba", "0_1_0");
}

#[test]
fn test_library_call() {
    let world = deploy_world();
    let world = world.dispatcher;

    world.register_library("dojo", library_a::TEST_CLASS_HASH.try_into().unwrap(), "liba", "0_1_0");

    let world = WorldStorageTrait::new(world, @"dojo");

    let (_, class_hash) = world.dns(@"liba_v0_1_0").unwrap();

    let liba = LibraryALibraryDispatcher { class_hash };
    let res = liba.get_byte();
    assert(res == 42, 'should return 42');
}

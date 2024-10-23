use dojo::world::Resource;
use dojo::world::world::Event;
use dojo::model::Model;
use dojo::utils::bytearray_hash;
use dojo::world::{
    IWorldDispatcher, IWorldDispatcherTrait, IUpgradeableWorldDispatcher,
    IUpgradeableWorldDispatcherTrait
};
use dojo::tests::helpers::{
    IbarDispatcher, IbarDispatcherTrait, drop_all_events, deploy_world_and_bar, Foo, foo, bar,
    Character, character, test_contract, test_contract_with_dojo_init_args, SimpleEvent,
    simple_event, SimpleEventEmitter
};
use dojo::utils::test::{spawn_test_world, deploy_with_world_address, GasCounterTrait};

#[starknet::interface]
trait IMetadataOnly<T> {
    fn selector(self: @T) -> felt252;
    fn name(self: @T) -> ByteArray;
    fn namespace(self: @T) -> ByteArray;
    fn namespace_hash(self: @T) -> felt252;
}

#[starknet::contract]
mod resource_metadata_malicious {
    use dojo::model::{ModelDefinition, ResourceMetadata};
    use dojo::utils::bytearray_hash;

    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl InvalidModelName of super::IMetadataOnly<ContractState> {
        fn selector(self: @ContractState) -> felt252 {
            ModelDefinition::<ResourceMetadata>::selector()
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

#[test]
#[available_gas(20000000)]
fn test_model() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
fn test_system() {
    let (world, bar_contract) = deploy_world_and_bar();

    bar_contract.set_foo(1337, 1337);

    let stored: Foo = get!(world, starknet::get_caller_address(), Foo);
    assert(stored.a == 1337, 'data not stored');
    assert(stored.b == 1337, 'data not stored');
}

#[test]
fn test_delete() {
    let (world, bar_contract) = deploy_world_and_bar();

    // set model
    bar_contract.set_foo(1337, 1337);
    let stored: Foo = get!(world, starknet::get_caller_address(), Foo);
    assert(stored.a == 1337, 'data not stored');
    assert(stored.b == 1337, 'data not stored');

    // delete model
    bar_contract.delete_foo_macro(stored);

    let deleted: Foo = get!(world, starknet::get_caller_address(), Foo);
    assert(deleted.a == 0, 'data not deleted');
    assert(deleted.b == 0, 'data not deleted');
}

#[test]
#[available_gas(6000000)]
fn test_contract_getter() {
    let world = deploy_world();

    let address = world
        .register_contract('salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap());

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

    let world = deploy_world();
    world.register_event(simple_event::TEST_CLASS_HASH.try_into().unwrap());
    world.grant_writer(dojo::event::Event::<SimpleEvent>::selector(), bob);

    drop_all_events(world.contract_address);

    starknet::testing::set_contract_address(bob);

    let simple_event = SimpleEvent { id: 2, data: (3, 4) };
    simple_event.emit(world);

    let event = starknet::testing::pop_log::<Event>(world.contract_address);

    assert(event.is_some(), 'no event');

    if let Event::EventEmitted(event) = event.unwrap() {
        assert(
            event.event_selector == dojo::event::Event::<SimpleEvent>::selector(),
            'bad event selector'
        );
        assert(event.system_address == bob, 'bad system address');
        assert(event.historical, 'bad historical value');
        assert(event.keys == [2].span(), 'bad keys');
        assert(event.values == [3, 4].span(), 'bad values');
    } else {
        core::panic_with_felt252('no EventEmitted event');
    }
}


// Utils
fn deploy_world() -> IWorldDispatcher {
    spawn_test_world(["dojo"].span(), [].span())
}

#[test]
fn test_execute_multiple_worlds() {
    // Deploy world contract
    let world1 = spawn_test_world(["dojo"].span(), [foo::TEST_CLASS_HASH].span(),);
    let contract_address = deploy_with_world_address(bar::TEST_CLASS_HASH, world1);

    world1.grant_writer(Model::<Foo>::selector(), contract_address);

    let bar1_contract = IbarDispatcher { contract_address };

    // Deploy another world contract
    let world2 = spawn_test_world(["dojo"].span(), [foo::TEST_CLASS_HASH].span(),);
    let contract_address = deploy_with_world_address(bar::TEST_CLASS_HASH, world2);

    world2.grant_writer(Model::<Foo>::selector(), contract_address);

    let bar2_contract = IbarDispatcher { contract_address };

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
    let (world, bar_contract) = deploy_world_and_bar();

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
    let world = spawn_test_world(["dojo"].span(), [character::TEST_CLASS_HASH].span(),);
    let contract_address = deploy_with_world_address(bar::TEST_CLASS_HASH, world);
    let bar_contract = IbarDispatcher { contract_address };

    world.grant_writer(Model::<Character>::selector(), contract_address);

    let alice = starknet::contract_address_const::<0xa11ce>();
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

    let admin = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_account_contract_address(admin);
    starknet::testing::set_contract_address(admin);

    let mut upgradeable_world_dispatcher = IUpgradeableWorldDispatcher {
        contract_address: world.contract_address
    };
    upgradeable_world_dispatcher.upgrade(0.try_into().unwrap());
}

#[test]
#[available_gas(60000000)]
#[should_panic(
    expected: ("Caller `4919` cannot upgrade the resource `0` (not owner)", 'ENTRYPOINT_FAILED')
)]
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

#[test]
#[available_gas(6000000)]
fn test_constructor_default() {
    let world = deploy_world();
    let _address = world
        .register_contract('salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
fn test_can_call_init_only_world() {
    let world = deploy_world();
    let address = world
        .register_contract('salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap());

    let expected_panic: ByteArray =
        "Only the world can init contract `dojo-test_contract`, but caller is `0`";

    match starknet::syscalls::call_contract_syscall(
        address, dojo::world::world::DOJO_INIT_SELECTOR, [].span()
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
        }
    }
}

#[test]
#[available_gas(6000000)]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED'))]
fn test_can_call_init_only_owner() {
    let world = deploy_world();
    let _address = world
        .register_contract('salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap());

    let bob = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(bob);

    world.init_contract(selector_from_tag!("dojo-test_contract"), [].span());
}

#[test]
#[available_gas(6000000)]
fn test_can_call_init_default() {
    let world = deploy_world();
    let _address = world
        .register_contract('salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap());

    world.init_contract(selector_from_tag!("dojo-test_contract"), [].span());
}

#[test]
#[available_gas(6000000)]
fn test_can_call_init_args() {
    let world = deploy_world();
    let _address = world
        .register_contract(
            'salt1', test_contract_with_dojo_init_args::TEST_CLASS_HASH.try_into().unwrap()
        );

    world.init_contract(selector_from_tag!("dojo-test_contract_with_dojo_init_args"), [1].span());
}

#[test]
fn test_can_call_init_only_world_args() {
    let world = deploy_world();
    let address = world
        .register_contract(
            'salt1', test_contract_with_dojo_init_args::TEST_CLASS_HASH.try_into().unwrap()
        );

    let expected_panic: ByteArray =
        "Only the world can init contract `dojo-test_contract_with_dojo_init_args`, but caller is `0`";

    match starknet::syscalls::call_contract_syscall(
        address, dojo::world::world::DOJO_INIT_SELECTOR, [123].span()
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
        }
    }
}

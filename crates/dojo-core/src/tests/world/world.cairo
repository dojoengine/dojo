use starknet::{contract_address_const, ContractAddress, get_caller_address};

use dojo::world::Resource;
use dojo::world::config::Config::{DifferProgramHashUpdate, FactsRegistryUpdate};
use dojo::world::config::{IConfigDispatcher, IConfigDispatcherTrait};
use dojo::model::{Model, ResourceMetadata};
use dojo::utils::bytearray_hash;
use dojo::world::{
    IWorldDispatcher, IWorldDispatcherTrait, world, IUpgradeableWorld, IUpgradeableWorldDispatcher,
    IUpgradeableWorldDispatcherTrait
};
use dojo::tests::helpers::{
    IbarDispatcher, IbarDispatcherTrait, drop_all_events, deploy_world_and_bar, Foo, foo, bar,
    Character, character, test_contract, test_contract_with_dojo_init_args
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

    let _ = world.deploy_contract('salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap(),);

    if let Resource::Contract((class_hash, _)) = world
        .resource(selector_from_tag!("dojo-test_contract")) {
        assert(
            class_hash == test_contract::TEST_CLASS_HASH.try_into().unwrap(),
            'invalid contract class hash'
        );
    }
}

#[test]
#[available_gas(6000000)]
fn test_model_class_hash_getter() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    if let Resource::Model((class_hash, _)) = world.resource(Model::<Foo>::selector()) {
        assert(class_hash == foo::TEST_CLASS_HASH.try_into().unwrap(), 'foo wrong class hash');
    } else {
        panic!("Foo model not found");
    };
}

#[test]
#[ignore]
#[available_gas(6000000)]
fn test_legacy_model_class_hash_getter() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    if let Resource::Model((class_hash, _)) = world.resource('Foo') {
        assert(class_hash == foo::TEST_CLASS_HASH.try_into().unwrap(), 'foo wrong class hash');
    } else {
        panic!("Foo model not found");
    };
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

use test_contract::IDojoInitDispatcherTrait;

#[test]
#[available_gas(6000000)]
#[should_panic(
    expected: (
        "Only the world can init contract `dojo-test_contract`, but caller is `0`",
        'ENTRYPOINT_FAILED'
    )
)]
fn test_can_call_init_only_world() {
    let world = deploy_world();
    let address = world
        .deploy_contract('salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap());

    let d = test_contract::IDojoInitDispatcher { contract_address: address };
    d.dojo_init();
}

#[test]
#[available_gas(6000000)]
#[should_panic(
    expected: (
        "Caller `4919` cannot initialize contract `dojo-test_contract` (not owner)",
        'ENTRYPOINT_FAILED'
    )
)]
fn test_can_call_init_only_owner() {
    let world = deploy_world();
    let _address = world
        .deploy_contract('salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap());

    let bob = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(bob);

    world.init_contract(selector_from_tag!("dojo-test_contract"), [].span());
}

#[test]
#[available_gas(6000000)]
fn test_can_call_init_default() {
    let world = deploy_world();
    let _address = world
        .deploy_contract('salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap());

    world.init_contract(selector_from_tag!("dojo-test_contract"), [].span());
}

#[test]
#[available_gas(6000000)]
fn test_can_call_init_args() {
    let world = deploy_world();
    let _address = world
        .deploy_contract(
            'salt1', test_contract_with_dojo_init_args::TEST_CLASS_HASH.try_into().unwrap()
        );

    world.init_contract(selector_from_tag!("dojo-test_contract_with_dojo_init_args"), [1].span());
}

use test_contract_with_dojo_init_args::IDojoInitDispatcherTrait as IDojoInitArgs;

#[test]
#[available_gas(6000000)]
#[should_panic(
    expected: (
        "Only the world can init contract `dojo-test_contract_with_dojo_init_args`, but caller is `0`",
        'ENTRYPOINT_FAILED'
    )
)]
fn test_can_call_init_only_world_args() {
    let world = deploy_world();
    let address = world
        .deploy_contract(
            'salt1', test_contract_with_dojo_init_args::TEST_CLASS_HASH.try_into().unwrap()
        );

    let d = test_contract_with_dojo_init_args::IDojoInitDispatcher { contract_address: address };
    d.dojo_init(123);
}

use dojo::world::update::IUpgradeableStateDispatcherTrait;

#[test]
#[available_gas(6000000)]
#[should_panic(
    expected: ("Caller `4919` can't upgrade state (not world owner)", 'ENTRYPOINT_FAILED')
)]
fn test_upgrade_state_not_owner() {
    let world = deploy_world();

    let not_owner = starknet::contract_address_const::<0x1337>();
    starknet::testing::set_contract_address(not_owner);
    starknet::testing::set_account_contract_address(not_owner);

    let output = dojo::world::update::ProgramOutput {
        prev_state_root: 0,
        new_state_root: 0,
        block_number: 0,
        block_hash: 0,
        config_hash: 0,
        world_da_hash: 0,
        message_to_starknet_segment: [].span(),
        message_to_appchain_segment: [].span(),
    };

    let d = dojo::world::update::IUpgradeableStateDispatcher {
        contract_address: world.contract_address
    };
    d.upgrade_state([].span(), output, 0);
}

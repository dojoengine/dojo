use starknet::{contract_address_const, ContractAddress, ClassHash};

use dojo::model::{Model, ResourceMetadata};
use dojo::utils::{bytearray_hash, entity_id_from_keys};
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait, world};
use dojo::world::world::{
    NamespaceRegistered, ModelRegistered, ModelUpgraded, MetadataUpdate, ContractDeployed,
    ContractUpgraded
};
use dojo::contract::{IContractDispatcher, IContractDispatcherTrait};

use dojo::tests::helpers::{
    deploy_world, drop_all_events, Foo, foo, Buzz, buzz, test_contract, buzz_contract
};
use dojo::utils::test::spawn_test_world;

#[test]
fn test_set_metadata_world() {
    let world = deploy_world();

    let metadata = ResourceMetadata {
        resource_id: 0, metadata_uri: format!("ipfs:world_with_a_long_uri_that")
    };

    world.set_metadata(metadata.clone());

    assert(world.metadata(0) == metadata, 'invalid metadata');
}

#[test]
fn test_set_metadata_resource_owner() {
    let world = spawn_test_world(["dojo"].span(), [foo::TEST_CLASS_HASH].span(),);

    let bob = starknet::contract_address_const::<0xb0b>();

    world.grant_owner(Model::<Foo>::selector(), bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let metadata = ResourceMetadata {
        resource_id: Model::<Foo>::selector(), metadata_uri: format!("ipfs:bob")
    };

    drop_all_events(world.contract_address);

    // Metadata must be updated by a direct call from an account which has owner role
    // for the attached resource.
    world.set_metadata(metadata.clone());
    assert(world.metadata(Model::<Foo>::selector()) == metadata, 'bad metadata');

    assert_eq!(
        starknet::testing::pop_log(world.contract_address),
        Option::Some(MetadataUpdate { resource: metadata.resource_id, uri: metadata.metadata_uri })
    );
}

#[test]
#[should_panic(
    expected: (
        "Caller `2827` is not the owner of the resource `3123252206139358744730647958636922105676576163624049771737508399526017186883`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_set_metadata_not_possible_for_resource_writer() {
    let world = spawn_test_world(["dojo"].span(), [foo::TEST_CLASS_HASH].span(),);

    let bob = starknet::contract_address_const::<0xb0b>();

    world.grant_writer(Model::<Foo>::selector(), bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let metadata = ResourceMetadata {
        resource_id: Model::<Foo>::selector(), metadata_uri: format!("ipfs:bob")
    };

    world.set_metadata(metadata.clone());
}

#[test]
#[should_panic(
    expected: ("Caller `2827` is not the owner of the resource `0`", 'ENTRYPOINT_FAILED',)
)]
fn test_set_metadata_not_possible_for_random_account() {
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
#[should_panic(
    expected: (
        "Caller `57005` is not the owner of the resource `3123252206139358744730647958636922105676576163624049771737508399526017186883`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_set_metadata_through_malicious_contract() {
    let world = spawn_test_world(["dojo"].span(), [foo::TEST_CLASS_HASH].span(),);

    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    world.grant_owner(Model::<Foo>::selector(), bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(malicious_contract);

    let metadata = ResourceMetadata {
        resource_id: Model::<Foo>::selector(), metadata_uri: format!("ipfs:bob")
    };

    world.set_metadata(metadata.clone());
}

#[test]
fn test_register_model_for_namespace_owner() {
    let bob = starknet::contract_address_const::<0xb0b>();

    let world = deploy_world();
    world.grant_owner(Model::<Foo>::namespace_hash(), bob);

    drop_all_events(world.contract_address);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<ModelRegistered>(world.contract_address);

    assert(event.is_some(), 'no ModelRegistered event');
    let event = event.unwrap();
    assert(event.name == Model::<Foo>::name(), 'bad model name');
    assert(event.namespace == Model::<Foo>::namespace(), 'bad model namespace');
    assert(event.class_hash == foo::TEST_CLASS_HASH.try_into().unwrap(), 'bad model class_hash');
    assert(
        event.address != core::num::traits::Zero::<ContractAddress>::zero(),
        'bad model prev address'
    );

    assert(world.is_owner(Model::<Foo>::selector(), bob), 'bob is not the owner');
}

#[test]
fn test_register_model_for_namespace_writer() {
    let bob = starknet::contract_address_const::<0xb0b>();

    let world = deploy_world();
    world.grant_writer(Model::<Foo>::namespace_hash(), bob);

    drop_all_events(world.contract_address);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<ModelRegistered>(world.contract_address);

    assert(event.is_some(), 'no ModelRegistered event');
    let event = event.unwrap();
    assert(event.name == Model::<Foo>::name(), 'bad model name');
    assert(event.namespace == Model::<Foo>::namespace(), 'bad model namespace');
    assert(event.class_hash == foo::TEST_CLASS_HASH.try_into().unwrap(), 'bad model class_hash');
    assert(
        event.address != core::num::traits::Zero::<ContractAddress>::zero(),
        'bad model prev address'
    );

    assert(world.is_owner(Model::<Foo>::selector(), bob), 'bob is not the owner');
}

#[test]
fn test_upgrade_model_from_model_owner() {
    let bob = starknet::contract_address_const::<0xb0b>();

    let world = deploy_world();
    world.grant_owner(Model::<Foo>::namespace_hash(), bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    drop_all_events(world.contract_address);

    world.upgrade_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let event = starknet::testing::pop_log::<ModelUpgraded>(world.contract_address);

    assert(event.is_some(), 'no ModelRegistered event');
    let event = event.unwrap();
    assert(event.name == Model::<Foo>::name(), 'bad model name');
    assert(event.namespace == Model::<Foo>::namespace(), 'bad model namespace');
    assert(event.class_hash == foo::TEST_CLASS_HASH.try_into().unwrap(), 'bad model class_hash');
    assert(
        event.address != core::num::traits::Zero::<ContractAddress>::zero(),
        'bad model prev address'
    );

    assert(world.is_owner(Model::<Foo>::selector(), bob), 'bob is not the owner');
}

#[test]
#[should_panic(
    expected: (
        "Caller `2827` cannot upgrade the resource `3123252206139358744730647958636922105676576163624049771737508399526017186883` (not owner)",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_upgrade_model_from_model_writer() {
    let bob = starknet::contract_address_const::<0xb0b>();
    let alice = starknet::contract_address_const::<0xa11ce>();

    let world = deploy_world();
    // dojo namespace is registered by the deploy_world function.
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    world.grant_owner(Model::<Foo>::namespace_hash(), bob);
    world.grant_writer(Model::<Foo>::namespace_hash(), alice);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.upgrade_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(expected: ("Resource `dojo-Foo` is already registered", 'ENTRYPOINT_FAILED',))]
fn test_upgrade_model_from_random_account() {
    let bob = starknet::contract_address_const::<0xb0b>();
    let alice = starknet::contract_address_const::<0xa11ce>();

    let world = deploy_world();
    world.grant_writer(Model::<Foo>::namespace_hash(), bob);
    world.grant_writer(Model::<Foo>::namespace_hash(), alice);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
#[should_panic(expected: ("Namespace `another_namespace` is not registered", 'ENTRYPOINT_FAILED',))]
fn test_register_model_with_unregistered_namespace() {
    let world = deploy_world();
    world.register_model(buzz::TEST_CLASS_HASH.try_into().unwrap());
}

// It's CONTRACT_NOT_DEPLOYED for now as in this example the contract is not a dojo contract
// and it's not the account that is calling the register_model function.
#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED',))]
fn test_register_model_through_malicious_contract() {
    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    let world = deploy_world();
    world.grant_owner(Model::<Foo>::namespace_hash(), bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(malicious_contract);
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
fn test_register_namespace() {
    let world = deploy_world();

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    drop_all_events(world.contract_address);

    let namespace = "namespace";
    let hash = bytearray_hash(@namespace);

    world.register_namespace(namespace);

    assert(world.is_owner(hash, bob), 'namespace not registered');

    assert_eq!(
        starknet::testing::pop_log(world.contract_address),
        Option::Some(NamespaceRegistered { namespace: "namespace", hash })
    );
}

#[test]
#[should_panic(expected: ("Namespace `namespace` is already registered", 'ENTRYPOINT_FAILED',))]
fn test_register_namespace_already_registered_same_caller() {
    let world = deploy_world();

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    world.register_namespace("namespace");
    world.register_namespace("namespace");
}

#[test]
#[should_panic(expected: ("Namespace `namespace` is already registered", 'ENTRYPOINT_FAILED',))]
fn test_register_namespace_already_registered_other_caller() {
    let world = deploy_world();

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    world.register_namespace("namespace");

    let alice = starknet::contract_address_const::<0xa11ce>();
    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.register_namespace("namespace");
}

#[test]
fn test_deploy_contract_for_namespace_owner() {
    let world = deploy_world();
    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let bob = starknet::contract_address_const::<0xb0b>();
    world.grant_owner(bytearray_hash(@"dojo"), bob);

    // the account owns the 'test_contract' namespace so it should be able to deploy the contract.
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    drop_all_events(world.contract_address);

    let contract_address = world.deploy_contract('salt1', class_hash);
    let dispatcher = IContractDispatcher { contract_address };

    let event = starknet::testing::pop_log::<ContractDeployed>(world.contract_address);
    assert(event.is_some(), 'no ContractDeployed event');
    let event = event.unwrap();
    assert(event.salt == 'salt1', 'bad event salt');
    assert(event.class_hash == class_hash, 'bad class_hash');
    assert(event.name == dispatcher.contract_name(), 'bad contract name');
    assert(event.namespace == dispatcher.namespace(), 'bad namespace');
    assert(
        event.address != core::num::traits::Zero::<ContractAddress>::zero(), 'bad contract address'
    );
}

#[test]
fn test_deploy_contract_for_namespace_writer() {
    let world = deploy_world();

    let bob = starknet::contract_address_const::<0xb0b>();
    world.grant_writer(bytearray_hash(@"dojo"), bob);

    // the account has write access to the 'test_contract' namespace so it should be able to deploy
    // the contract.
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    world.deploy_contract('salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap(),);
}


#[test]
#[should_panic(
    expected: ("Caller `2827` has no write access on namespace `dojo`", 'ENTRYPOINT_FAILED',)
)]
fn test_deploy_contract_no_namespace_write_access() {
    let world = deploy_world();

    let bob = starknet::contract_address_const::<0xb0b>();
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    world.deploy_contract('salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap(),);
}

#[test]
#[should_panic(expected: ("Namespace `buzz_namespace` is not registered", 'ENTRYPOINT_FAILED',))]
fn test_deploy_contract_with_unregistered_namespace() {
    let world = deploy_world();
    world.deploy_contract('salt1', buzz_contract::TEST_CLASS_HASH.try_into().unwrap(),);
}

// It's CONTRACT_NOT_DEPLOYED for now as in this example the contract is not a dojo contract
// and it's not the account that is calling the deploy_contract function.
#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED',))]
fn test_deploy_contract_through_malicious_contract() {
    let world = deploy_world();

    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    world.grant_owner(bytearray_hash(@"dojo"), bob);

    // the account owns the 'test_contract' namespace so it should be able to deploy the contract.
    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(malicious_contract);

    world.deploy_contract('salt1', test_contract::TEST_CLASS_HASH.try_into().unwrap(),);
}

#[test]
fn test_upgrade_contract_from_resource_owner() {
    let world = deploy_world();
    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let bob = starknet::contract_address_const::<0xb0b>();

    world.grant_owner(bytearray_hash(@"dojo"), bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let contract_address = world.deploy_contract('salt1', class_hash);

    let dispatcher = IContractDispatcher { contract_address };

    drop_all_events(world.contract_address);

    world.upgrade_contract(dispatcher.selector(), class_hash);

    let event = starknet::testing::pop_log::<ContractUpgraded>(world.contract_address);
    assert(event.is_some(), 'no ContractUpgraded event');
    let event = event.unwrap();
    assert(event.class_hash == class_hash, 'bad class_hash');
    assert(
        event.address != core::num::traits::Zero::<ContractAddress>::zero(), 'bad contract address'
    );
}

#[test]
#[should_panic(
    expected: (
        "Caller `659918` is not the owner of the resource `2368393732245529956313345237151518608283468650081902115301417183793437311044`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_upgrade_contract_from_resource_writer() {
    let world = deploy_world();
    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let bob = starknet::contract_address_const::<0xb0b>();
    let alice = starknet::contract_address_const::<0xa11ce>();

    world.grant_owner(bytearray_hash(@"dojo"), bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let contract_address = world.deploy_contract('salt1', class_hash);

    let dispatcher = IContractDispatcher { contract_address };

    world.grant_writer(dispatcher.selector(), alice);

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.upgrade_contract(dispatcher.selector(), class_hash);
}

#[test]
#[should_panic(
    expected: (
        "Caller `659918` is not the owner of the resource `2368393732245529956313345237151518608283468650081902115301417183793437311044`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_upgrade_contract_from_random_account() {
    let world = deploy_world();
    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let contract_address = world.deploy_contract('salt1', class_hash);

    let alice = starknet::contract_address_const::<0xa11ce>();
    let dispatcher = IContractDispatcher { contract_address };

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.upgrade_contract(dispatcher.selector(), class_hash);
}

#[test]
#[should_panic(
    expected: (
        "Caller `57005` is not the owner of the resource `2368393732245529956313345237151518608283468650081902115301417183793437311044`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_upgrade_contract_through_malicious_contract() {
    let world = deploy_world();
    let class_hash = test_contract::TEST_CLASS_HASH.try_into().unwrap();

    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    world.grant_owner(bytearray_hash(@"dojo"), bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let contract_address = world.deploy_contract('salt1', class_hash);

    let dispatcher = IContractDispatcher { contract_address };

    starknet::testing::set_contract_address(malicious_contract);

    world.upgrade_contract(dispatcher.selector(), class_hash);
}

use dojo::world::{world, IWorldDispatcherTrait};
use dojo::model::{Model, ResourceMetadata};

use crate::tests::helpers::{DOJO_NSH, Foo, drop_all_events, deploy_world, deploy_world_and_foo};

#[test]
fn test_set_metadata_world() {
    let world = deploy_world();
    let world = world.dispatcher;

    let metadata = ResourceMetadata {
        resource_id: 0, metadata_uri: format!("ipfs:world_with_a_long_uri_that"), metadata_hash: 42,
    };

    world.set_metadata(metadata.clone());

    assert(world.metadata(0) == metadata, 'invalid metadata');
}

#[test]
fn test_set_metadata_resource_owner() {
    let (world, model_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();

    world.grant_owner(Model::<Foo>::selector(DOJO_NSH), bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let metadata = ResourceMetadata {
        resource_id: model_selector, metadata_uri: format!("ipfs:bob"), metadata_hash: 42,
    };

    drop_all_events(world.contract_address);

    // Metadata must be updated by a direct call from an account which has owner role
    // for the attached resource.
    world.set_metadata(metadata.clone());
    assert(world.metadata(model_selector) == metadata, 'bad metadata');

    let event = starknet::testing::pop_log::<world::Event>(world.contract_address);
    assert(event.is_some(), 'no event)');

    if let world::Event::MetadataUpdate(event) = event.unwrap() {
        assert(event.resource == metadata.resource_id, 'bad resource');
        assert(event.uri == metadata.metadata_uri, 'bad uri');
        assert(event.hash == metadata.metadata_hash, 'bad hash');
    } else {
        core::panic_with_felt252('no EventUpgraded event');
    }
}

#[test]
#[should_panic(
    expected: (
        "Account `0xb0b` does NOT have OWNER role on model (or its namespace) `Foo`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_set_metadata_not_possible_for_resource_writer() {
    let (world, model_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();

    world.grant_writer(model_selector, bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(bob);

    let metadata = ResourceMetadata {
        resource_id: model_selector, metadata_uri: format!("ipfs:bob"), metadata_hash: 42,
    };

    world.set_metadata(metadata.clone());
}

#[test]
#[should_panic(
    expected: ("Account `0xb0b` does NOT have OWNER role on world", 'ENTRYPOINT_FAILED'),
)]
fn test_set_metadata_not_possible_for_random_account() {
    let world = deploy_world();
    let world = world.dispatcher;

    let metadata = ResourceMetadata { // World metadata.
        resource_id: 0, metadata_uri: format!("ipfs:bob"), metadata_hash: 42,
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
        "Contract `0xdead` does NOT have OWNER role on model (or its namespace) `Foo`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_set_metadata_through_malicious_contract() {
    let (world, model_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    world.grant_owner(model_selector, bob);

    starknet::testing::set_account_contract_address(bob);
    starknet::testing::set_contract_address(malicious_contract);

    let metadata = ResourceMetadata {
        resource_id: model_selector, metadata_uri: format!("ipfs:bob"), metadata_hash: 42,
    };

    world.set_metadata(metadata.clone());
}

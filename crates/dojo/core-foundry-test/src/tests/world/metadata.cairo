use dojo::world::{world, IWorldDispatcherTrait};
use dojo::model::{Model, ResourceMetadata};

use crate::tests::helpers::{DOJO_NSH, Foo, deploy_world, deploy_world_and_foo};
use crate::snf_utils;

use snforge_std::{spy_events, EventSpyAssertionsTrait};

#[test]
fn test_set_metadata_world() {
    // deploy a dedicated contract to be used as caller/account address because of
    // the way `world.panic_with_details()` is written.
    // Once this function will use SRC5, we will be able to remove these lines
    let caller_contract = snf_utils::declare_and_deploy("dojo_caller_contract");
    snf_utils::set_caller_address(caller_contract);
    snf_utils::set_account_address(caller_contract);

    let world = deploy_world();
    let world = world.dispatcher;

    let metadata = ResourceMetadata {
        resource_id: 0, metadata_uri: format!("ipfs:world_with_a_long_uri_that"), metadata_hash: 42
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

    snf_utils::set_account_address(bob);
    snf_utils::set_caller_address(bob);

    let metadata = ResourceMetadata {
        resource_id: model_selector, metadata_uri: format!("ipfs:bob"), metadata_hash: 42
    };

    let mut spy = spy_events();

    // Metadata must be updated by a direct call from an account which has owner role
    // for the attached resource.
    world.set_metadata(metadata.clone());
    assert(world.metadata(model_selector) == metadata, 'bad metadata');

    spy
        .assert_emitted(
            @array![
                (
                    world.contract_address,
                    world::Event::MetadataUpdate(
                        world::MetadataUpdate {
                            resource: metadata.resource_id,
                            uri: metadata.metadata_uri,
                            hash: metadata.metadata_hash
                        }
                    )
                )
            ]
        );
}

#[test]
#[should_panic(
    expected: "Account `2827` does NOT have OWNER role on model (or its namespace) `Foo`"
)]
fn test_set_metadata_not_possible_for_resource_writer() {
    let (world, model_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();

    world.grant_writer(model_selector, bob);

    snf_utils::set_account_address(bob);
    snf_utils::set_caller_address(bob);

    let metadata = ResourceMetadata {
        resource_id: model_selector, metadata_uri: format!("ipfs:bob"), metadata_hash: 42
    };

    world.set_metadata(metadata.clone());
}

#[test]
#[should_panic(expected: "Account `2827` does NOT have OWNER role on world")]
fn test_set_metadata_not_possible_for_random_account() {
    let world = deploy_world();
    let world = world.dispatcher;

    let metadata = ResourceMetadata { // World metadata.
        resource_id: 0, metadata_uri: format!("ipfs:bob"), metadata_hash: 42
    };

    let bob = starknet::contract_address_const::<0xb0b>();
    snf_utils::set_caller_address(bob);
    snf_utils::set_account_address(bob);

    // Bob access follows the conventional ACL, he can't write the world
    // metadata if he does not have access to it.
    world.set_metadata(metadata);
}

#[test]
#[should_panic(expected: ('ENTRYPOINT_NOT_FOUND', 'ENTRYPOINT_FAILED'))]
fn test_set_metadata_through_malicious_contract() {
    let (world, model_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = snf_utils::declare_and_deploy("malicious_contract");

    world.grant_owner(model_selector, bob);

    snf_utils::set_account_address(bob);
    snf_utils::set_caller_address(malicious_contract);

    let metadata = ResourceMetadata {
        resource_id: model_selector, metadata_uri: format!("ipfs:bob"), metadata_hash: 42
    };

    world.set_metadata(metadata.clone());
}

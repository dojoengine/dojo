use dojo::utils::bytearray_hash;
use dojo::world::IWorldDispatcherTrait;

use crate::tests::helpers::{
    deploy_world, foo_setter, IFooSetterDispatcher, IFooSetterDispatcherTrait, deploy_world_and_foo,
    deploy_world_with_all_kind_of_resources,
};
use crate::tests::expanded::selector_attack::{attacker_model, attacker_contract};

#[test]
fn test_owner() {
    let (world, foo_selector) = deploy_world_and_foo();

    let world = world.dispatcher;

    let alice = starknet::contract_address_const::<0xa11ce>();
    let bob = starknet::contract_address_const::<0xb0b>();

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
#[should_panic(expected: ("Resource `42` is not registered", 'ENTRYPOINT_FAILED'))]
fn test_grant_owner_not_registered_resource() {
    let world = deploy_world();
    let world = world.dispatcher;

    // 42 is not a registered resource ID
    world.grant_owner(42, 69.try_into().unwrap());
}

#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED', 'ENTRYPOINT_FAILED'))]
fn test_grant_owner_through_malicious_contract() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice = starknet::contract_address_const::<0xa11ce>();
    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    world.grant_owner(foo_selector, alice);

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(malicious_contract);

    world.grant_owner(foo_selector, bob);
}

#[test]
#[should_panic(
    expected: (
        "Account `659918` does NOT have OWNER role on model (or its namespace) `Foo`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_grant_owner_fails_for_non_owner() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice = starknet::contract_address_const::<0xa11ce>();
    let bob = starknet::contract_address_const::<0xb0b>();

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.grant_owner(foo_selector, bob);
}

#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED', 'ENTRYPOINT_FAILED'))]
fn test_revoke_owner_through_malicious_contract() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice = starknet::contract_address_const::<0xa11ce>();
    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    world.grant_owner(foo_selector, alice);
    world.grant_owner(foo_selector, bob);

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(malicious_contract);

    world.revoke_owner(foo_selector, bob);
}

#[test]
#[should_panic(
    expected: (
        "Account `659918` does NOT have OWNER role on model (or its namespace) `Foo`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_revoke_owner_fails_for_non_owner() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice = starknet::contract_address_const::<0xa11ce>();
    let bob = starknet::contract_address_const::<0xb0b>();

    world.grant_owner(foo_selector, bob);

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.revoke_owner(foo_selector, bob);
}

#[test]
#[available_gas(6000000)]
fn test_writer() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    assert(!world.is_writer(foo_selector, 69.try_into().unwrap()), 'should not be writer');

    world.grant_writer(foo_selector, 69.try_into().unwrap());
    assert(world.is_writer(foo_selector, 69.try_into().unwrap()), 'should be writer');

    world.revoke_writer(foo_selector, 69.try_into().unwrap());
    assert(!world.is_writer(foo_selector, 69.try_into().unwrap()), 'should not be writer');
}

#[test]
fn test_writer_not_registered_resource() {
    let world = deploy_world();
    let world = world.dispatcher;

    // 42 is not a registered resource ID
    !world.is_writer(42, 69.try_into().unwrap());
}

#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED', 'ENTRYPOINT_FAILED'))]
fn test_grant_writer_through_malicious_contract() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice = starknet::contract_address_const::<0xa11ce>();
    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    world.grant_owner(foo_selector, alice);

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(malicious_contract);

    world.grant_writer(foo_selector, bob);
}

#[test]
#[should_panic(
    expected: (
        "Account `659918` does NOT have OWNER role on model (or its namespace) `Foo`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_grant_writer_fails_for_non_owner() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice = starknet::contract_address_const::<0xa11ce>();
    let bob = starknet::contract_address_const::<0xb0b>();

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.grant_writer(foo_selector, bob);
}

#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED', 'ENTRYPOINT_FAILED'))]
fn test_revoke_writer_through_malicious_contract() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice = starknet::contract_address_const::<0xa11ce>();
    let bob = starknet::contract_address_const::<0xb0b>();
    let malicious_contract = starknet::contract_address_const::<0xdead>();

    world.grant_owner(foo_selector, alice);
    world.grant_writer(foo_selector, bob);

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(malicious_contract);

    world.revoke_writer(foo_selector, bob);
}

#[test]
#[should_panic(
    expected: (
        "Account `659918` does NOT have OWNER role on model (or its namespace) `Foo`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_revoke_writer_fails_for_non_owner() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice = starknet::contract_address_const::<0xa11ce>();
    let bob = starknet::contract_address_const::<0xb0b>();

    world.grant_owner(foo_selector, bob);

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.revoke_writer(foo_selector, bob);
}

#[test]
#[should_panic(
    expected: (
        "Contract `foo_setter` does NOT have WRITER role on model (or its namespace) `Foo`",
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_not_writer_with_known_contract() {
    let (world, _) = deploy_world_and_foo();
    let world = world.dispatcher;

    let account = starknet::contract_address_const::<0xb0b>();
    world.grant_owner(bytearray_hash(@"dojo"), account);

    // the account owns the 'test_contract' namespace so it should be able to deploy
    // and register the model.
    starknet::testing::set_account_contract_address(account);
    starknet::testing::set_contract_address(account);

    let contract_address = world
        .register_contract('salt1', "dojo", foo_setter::TEST_CLASS_HASH.try_into().unwrap());

    let d = IFooSetterDispatcher { contract_address };
    d.set_foo(1, 2);

    core::panics::panic_with_byte_array(
        @"Contract `dojo-foo_setter` does NOT have WRITER role on model (or its namespace) `Foo`",
    );
}

/// Test that an attacker can't control the hashes of resources in other namespaces
/// by registering a model in an other namespace.
#[test]
#[should_panic(
    expected: (
        "Account `7022365680606078322` does NOT have OWNER role on namespace `dojo`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_register_model_namespace_not_owner() {
    let owner = starknet::contract_address_const::<'owner'>();
    let attacker = starknet::contract_address_const::<'attacker'>();

    starknet::testing::set_account_contract_address(owner);
    starknet::testing::set_contract_address(owner);

    // Owner deploys the world and register Foo model.
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    assert(world.is_owner(foo_selector, owner), 'should be owner');

    starknet::testing::set_contract_address(attacker);
    starknet::testing::set_account_contract_address(attacker);

    // Attacker has control over the this namespace.
    world.register_namespace("atk");

    // Attacker can't take ownership of the Foo model in the dojo namespace.
    world.register_model("dojo", attacker_model::TEST_CLASS_HASH.try_into().unwrap());
}

/// Test that an attacker can't control the hashes of resources in other namespaces
/// by deploying a contract in an other namespace.
#[test]
#[should_panic(
    expected: (
        "Account `7022365680606078322` does NOT have OWNER role on namespace `dojo`",
        'ENTRYPOINT_FAILED',
    ),
)]
fn test_register_contract_namespace_not_owner() {
    let owner = starknet::contract_address_const::<'owner'>();
    let attacker = starknet::contract_address_const::<'attacker'>();

    starknet::testing::set_account_contract_address(owner);
    starknet::testing::set_contract_address(owner);

    // Owner deploys the world and register Foo model.
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    assert(world.is_owner(foo_selector, owner), 'should be owner');

    starknet::testing::set_contract_address(attacker);
    starknet::testing::set_account_contract_address(attacker);

    // Attacker has control over the this namespace.
    world.register_namespace("atk");

    // Attacker can't take ownership of the Foo model.
    world
        .register_contract('salt1', "dojo", attacker_contract::TEST_CLASS_HASH.try_into().unwrap());
}

#[test]
fn test_get_number_of_owners() {
    let owner = starknet::contract_address_const::<'owner'>();
    starknet::testing::set_account_contract_address(owner);
    starknet::testing::set_contract_address(owner);

    let (world, resources) = deploy_world_with_all_kind_of_resources();
    let world = world.dispatcher;

    let bob = starknet::contract_address_const::<0xb0b>();

    assert(world.get_number_of_owners(0xa11ce) == 0, 'no owner for unknown resource');

    for resource in resources {
        let resource = *resource;

        // after world deployment, a resource has 1 owner
        assert(world.get_number_of_owners(resource) == 1, 'resource should have 1 owner');

        // granting ownership for an existing owner should NOT increase the number of owners
        world.grant_owner(resource, owner);
        assert(world.get_number_of_owners(resource) == 1, 'resource should have 1 owner');

        // granting ownership for new owner should increase the number of owners
        world.grant_owner(resource, bob);
        assert(world.get_number_of_owners(resource) == 2, 'resource should have 2 owners');

        // revoking ownership should decrease the number of owners
        world.revoke_owner(resource, bob);
        assert(world.get_number_of_owners(resource) == 1, 'resource should have 1 owner');

        // revoking ownership for an already revoked owner should NOT decrease the number of owners
        world.revoke_owner(resource, bob);
        assert(world.get_number_of_owners(resource) == 1, 'resource should have 1 owner');

        // revoking the last owner should set the number of owners to 0
        world.revoke_owner(resource, owner);
        assert(world.get_number_of_owners(resource) == 0, 'resource should have no owner');
    }
}

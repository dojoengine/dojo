use dojo::world::IWorldDispatcherTrait;
use dojo_snf_test;
use starknet::ContractAddress;
use crate::tests::helpers::{
    IFooSetterDispatcher, IFooSetterDispatcherTrait, deploy_world, deploy_world_and_foo,
    deploy_world_with_all_kind_of_resources,
};

#[test]
fn test_owner() {
    // deploy a dedicated contract to be used as caller/account address because of
    // the way `world.panic_with_details()` is written.
    // Once this function will use SRC5, we will be able to remove these lines
    let caller_contract = dojo_snf_test::declare_and_deploy("dojo_caller_contract");
    dojo_snf_test::set_caller_address(caller_contract);
    dojo_snf_test::set_account_address(caller_contract);

    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let test_contract = dojo_snf_test::declare_and_deploy("test_contract");
    let another_test_contract = dojo_snf_test::declare_and_deploy("another_test_contract");

    assert(!world.is_owner(0, test_contract), 'should not be owner');
    assert(!world.is_owner(foo_selector, another_test_contract), 'should not be owner');

    world.grant_owner(0, test_contract);
    assert(world.is_owner(0, test_contract), 'should be owner');

    world.grant_owner(foo_selector, another_test_contract);
    assert(world.is_owner(foo_selector, another_test_contract), 'should be owner');

    world.revoke_owner(0, test_contract);
    assert(!world.is_owner(0, test_contract), 'should not be owner');

    world.revoke_owner(foo_selector, another_test_contract);
    assert(!world.is_owner(foo_selector, another_test_contract), 'should not be owner');
}


#[test]
#[should_panic(expected: "Resource `42` is not registered")]
fn test_grant_owner_not_registered_resource() {
    let world = deploy_world();
    let world = world.dispatcher;

    // 42 is not a registered resource ID
    world.grant_owner(42, 69.try_into().unwrap());
}

#[test]
#[should_panic(
    expected: "Contract `0x1c31979af9015c7943497c5e384cacc5b4c7e7fac60d4fb5e2c708daff22bf6` does NOT have OWNER role on model (or its namespace) `Foo`",
)]
fn test_grant_owner_through_malicious_contract() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice: ContractAddress = 0xa11ce.try_into().unwrap();
    let bob: ContractAddress = 0xb0b.try_into().unwrap();
    let malicious_contract = dojo_snf_test::declare_and_deploy("malicious_contract");

    world.grant_owner(foo_selector, alice);

    dojo_snf_test::set_account_address(alice);
    dojo_snf_test::set_caller_address(malicious_contract);

    world.grant_owner(foo_selector, bob);
}

#[test]
#[should_panic(
    expected: "Account `0xa11ce` does NOT have OWNER role on model (or its namespace) `Foo`",
)]
fn test_grant_owner_fails_for_non_owner() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice: ContractAddress = 0xa11ce.try_into().unwrap();
    let bob: ContractAddress = 0xb0b.try_into().unwrap();

    dojo_snf_test::set_account_address(alice);
    dojo_snf_test::set_caller_address(alice);

    world.grant_owner(foo_selector, bob);
}

#[test]
#[should_panic(
    expected: "Contract `0x1c31979af9015c7943497c5e384cacc5b4c7e7fac60d4fb5e2c708daff22bf6` does NOT have OWNER role on model (or its namespace) `Foo`",
)]
fn test_revoke_owner_through_malicious_contract() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice: ContractAddress = 0xa11ce.try_into().unwrap();
    let bob: ContractAddress = 0xb0b.try_into().unwrap();
    let malicious_contract = dojo_snf_test::declare_and_deploy("malicious_contract");

    world.grant_owner(foo_selector, alice);
    world.grant_owner(foo_selector, bob);

    dojo_snf_test::set_account_address(alice);
    dojo_snf_test::set_caller_address(malicious_contract);

    world.revoke_owner(foo_selector, bob);
}

#[test]
#[should_panic(
    expected: "Account `0xa11ce` does NOT have OWNER role on model (or its namespace) `Foo`",
)]
fn test_revoke_owner_fails_for_non_owner() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice: ContractAddress = 0xa11ce.try_into().unwrap();
    let bob: ContractAddress = 0xb0b.try_into().unwrap();

    world.grant_owner(foo_selector, bob);

    dojo_snf_test::set_account_address(alice);
    dojo_snf_test::set_caller_address(alice);

    world.revoke_owner(foo_selector, bob);
}

#[test]
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
#[should_panic(
    expected: "Contract `0x1c31979af9015c7943497c5e384cacc5b4c7e7fac60d4fb5e2c708daff22bf6` does NOT have OWNER role on model (or its namespace) `Foo`",
)]
fn test_grant_writer_through_malicious_contract() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice: ContractAddress = 0xa11ce.try_into().unwrap();
    let bob: ContractAddress = 0xb0b.try_into().unwrap();
    let malicious_contract = dojo_snf_test::declare_and_deploy("malicious_contract");

    world.grant_owner(foo_selector, alice);

    dojo_snf_test::set_account_address(alice);
    dojo_snf_test::set_caller_address(malicious_contract);

    world.grant_writer(foo_selector, bob);
}

#[test]
#[should_panic(
    expected: "Account `0xa11ce` does NOT have OWNER role on model (or its namespace) `Foo`",
)]
fn test_grant_writer_fails_for_non_owner() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice: ContractAddress = 0xa11ce.try_into().unwrap();
    let bob: ContractAddress = 0xb0b.try_into().unwrap();

    dojo_snf_test::set_account_address(alice);
    dojo_snf_test::set_caller_address(alice);

    world.grant_writer(foo_selector, bob);
}

#[test]
#[should_panic(
    expected: "Contract `0x1c31979af9015c7943497c5e384cacc5b4c7e7fac60d4fb5e2c708daff22bf6` does NOT have OWNER role on model (or its namespace) `Foo`",
)]
fn test_revoke_writer_through_malicious_contract() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice: ContractAddress = 0xa11ce.try_into().unwrap();
    let bob: ContractAddress = 0xb0b.try_into().unwrap();
    let malicious_contract = dojo_snf_test::declare_and_deploy("malicious_contract");

    world.grant_owner(foo_selector, alice);
    world.grant_writer(foo_selector, bob);

    dojo_snf_test::set_account_address(alice);
    dojo_snf_test::set_caller_address(malicious_contract);

    world.revoke_writer(foo_selector, bob);
}

#[test]
#[should_panic(
    expected: "Account `0xa11ce` does NOT have OWNER role on model (or its namespace) `Foo`",
)]
fn test_revoke_writer_fails_for_non_owner() {
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    let alice: ContractAddress = 0xa11ce.try_into().unwrap();
    let bob: ContractAddress = 0xb0b.try_into().unwrap();

    world.grant_owner(foo_selector, bob);

    dojo_snf_test::set_account_address(alice);
    dojo_snf_test::set_caller_address(alice);

    world.revoke_writer(foo_selector, bob);
}

#[test]
#[should_panic(
    expected: "Contract `foo_setter` does NOT have WRITER role on model (or its namespace) `Foo`",
)]
fn test_not_writer_with_known_contract() {
    let (world, _) = deploy_world_and_foo();
    let world = world.dispatcher;

    let contract_address = world
        .register_contract('salt1', "dojo", dojo_snf_test::declare_contract("foo_setter"));

    let d = IFooSetterDispatcher { contract_address };
    d.set_foo(1, 2);
}

/// Test that an attacker can't control the hashes of resources in other namespaces
/// by registering a model in an other namespace.
#[test]
#[should_panic(
    expected: "Account `0x61747461636b6572` does NOT have OWNER role on namespace `dojo`",
)]
fn test_register_model_namespace_not_owner() {
    let owner: ContractAddress = 'owner'.try_into().unwrap();
    let attacker: ContractAddress = 'attacker'.try_into().unwrap();

    dojo_snf_test::set_account_address(owner);
    dojo_snf_test::set_caller_address(owner);

    // Owner deploys the world and register Foo model.
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    assert(world.is_owner(foo_selector, owner), 'should be owner');

    dojo_snf_test::set_caller_address(attacker);
    dojo_snf_test::set_account_address(attacker);

    // Attacker has control over the this namespace.
    world.register_namespace("atk");

    // Attacker can't take ownership of the Foo model in the dojo namespace.
    world.register_model("dojo", dojo_snf_test::declare_contract("attacker_model"));
}

/// Test that an attacker can't control the hashes of resources in other namespaces
/// by deploying a contract in an other namespace.
#[test]
#[should_panic(
    expected: "Account `0x61747461636b6572` does NOT have OWNER role on namespace `dojo`",
)]
fn test_register_contract_namespace_not_owner() {
    let owner: ContractAddress = 'owner'.try_into().unwrap();
    let attacker: ContractAddress = 'attacker'.try_into().unwrap();

    dojo_snf_test::set_account_address(owner);
    dojo_snf_test::set_caller_address(owner);

    // Owner deploys the world and register Foo model.
    let (world, foo_selector) = deploy_world_and_foo();
    let world = world.dispatcher;

    assert(world.is_owner(foo_selector, owner), 'should be owner');

    dojo_snf_test::set_caller_address(attacker);
    dojo_snf_test::set_account_address(attacker);

    // Attacker has control over the this namespace.
    world.register_namespace("atk");

    // Attacker can't take ownership of the Foo model.
    world.register_contract('salt1', "dojo", dojo_snf_test::declare_contract("attacker_contract"));
}

#[test]
fn test_owners_count() {
    let owner: ContractAddress = 'owner'.try_into().unwrap();
    let bob: ContractAddress = 'bob'.try_into().unwrap();

    dojo_snf_test::set_account_address(owner);
    dojo_snf_test::set_caller_address(owner);

    let (world, resources) = deploy_world_with_all_kind_of_resources();
    let world = world.dispatcher;

    assert(world.owners_count(0xa11ce) == 0, 'no owner for unknown resource');

    for resource in resources {
        let resource = *resource;

        // after world deployment, a resource has 1 owner
        assert(world.owners_count(resource) == 1, 'resource should have 1 owner');

        // granting ownership for an existing owner should NOT increase the number of owners
        world.grant_owner(resource, owner);
        assert(world.owners_count(resource) == 1, 'resource should have 1 owner');

        // granting ownership for new owner should increase the number of owners
        world.grant_owner(resource, bob);
        assert(world.owners_count(resource) == 2, 'resource should have 2 owners');

        // revoking ownership should decrease the number of owners
        world.revoke_owner(resource, bob);
        assert(world.owners_count(resource) == 1, 'resource should have 1 owner');

        // revoking ownership for an already revoked owner should NOT decrease the number of owners
        world.revoke_owner(resource, bob);
        assert(world.owners_count(resource) == 1, 'resource should have 1 owner');

        // revoking the last owner should set the number of owners to 0
        world.revoke_owner(resource, owner);
        assert(world.owners_count(resource) == 0, 'resource should have no owner');
    }
}

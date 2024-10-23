use dojo::model::Model;
use dojo::utils::bytearray_hash;
use dojo::world::IWorldDispatcherTrait;

use dojo::tests::helpers::{
    deploy_world, Foo, foo, foo_setter, IFooSetterDispatcher, IFooSetterDispatcherTrait
};
use dojo::tests::expanded::selector_attack::{attacker_contract, attacker_model};

#[test]
fn test_owner() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let foo_selector = Model::<Foo>::selector();

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

    // 42 is not a registered resource ID
    world.grant_owner(42, 69.try_into().unwrap());
}

#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED'))]
fn test_grant_owner_through_malicious_contract() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let foo_selector = Model::<Foo>::selector();

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
        "Account `659918` does NOT have OWNER role on model (or its namespace) `dojo-Foo`",
        'ENTRYPOINT_FAILED'
    )
)]
fn test_grant_owner_fails_for_non_owner() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let foo_selector = Model::<Foo>::selector();

    let alice = starknet::contract_address_const::<0xa11ce>();
    let bob = starknet::contract_address_const::<0xb0b>();

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.grant_owner(foo_selector, bob);
}

#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED'))]
fn test_revoke_owner_through_malicious_contract() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let foo_selector = Model::<Foo>::selector();

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
        "Account `659918` does NOT have OWNER role on model (or its namespace) `dojo-Foo`",
        'ENTRYPOINT_FAILED'
    )
)]
fn test_revoke_owner_fails_for_non_owner() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let foo_selector = Model::<Foo>::selector();

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
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let foo_selector = Model::<Foo>::selector();

    assert(!world.is_writer(foo_selector, 69.try_into().unwrap()), 'should not be writer');

    world.grant_writer(foo_selector, 69.try_into().unwrap());
    assert(world.is_writer(foo_selector, 69.try_into().unwrap()), 'should be writer');

    world.revoke_writer(foo_selector, 69.try_into().unwrap());
    assert(!world.is_writer(foo_selector, 69.try_into().unwrap()), 'should not be writer');
}

#[test]
fn test_writer_not_registered_resource() {
    let world = deploy_world();

    // 42 is not a registered resource ID
    !world.is_writer(42, 69.try_into().unwrap());
}

#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED'))]
fn test_grant_writer_through_malicious_contract() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let foo_selector = Model::<Foo>::selector();

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
        "Account `659918` does NOT have OWNER role on model (or its namespace) `dojo-Foo`",
        'ENTRYPOINT_FAILED'
    )
)]
fn test_grant_writer_fails_for_non_owner() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let foo_selector = Model::<Foo>::selector();

    let alice = starknet::contract_address_const::<0xa11ce>();
    let bob = starknet::contract_address_const::<0xb0b>();

    starknet::testing::set_account_contract_address(alice);
    starknet::testing::set_contract_address(alice);

    world.grant_writer(foo_selector, bob);
}

#[test]
#[should_panic(expected: ('CONTRACT_NOT_DEPLOYED', 'ENTRYPOINT_FAILED'))]
fn test_revoke_writer_through_malicious_contract() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let foo_selector = Model::<Foo>::selector();

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
        "Account `659918` does NOT have OWNER role on model (or its namespace) `dojo-Foo`",
        'ENTRYPOINT_FAILED'
    )
)]
fn test_revoke_writer_fails_for_non_owner() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());
    let foo_selector = Model::<Foo>::selector();

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
        "Contract `dojo-foo_setter` does NOT have WRITER role on model (or its namespace) `dojo-Foo`",
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED'
    )
)]
fn test_not_writer_with_known_contract() {
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let account = starknet::contract_address_const::<0xb0b>();
    world.grant_owner(bytearray_hash(@"dojo"), account);

    // the account owns the 'test_contract' namespace so it should be able to deploy
    // and register the model.
    starknet::testing::set_account_contract_address(account);
    starknet::testing::set_contract_address(account);

    let contract_address = world
        .register_contract('salt1', foo_setter::TEST_CLASS_HASH.try_into().unwrap());
    let d = IFooSetterDispatcher { contract_address };
    d.set_foo(1, 2);
}

/// Test that an attacker can't control the hashes of resources in other namespaces
/// by registering a model in an other namespace.
#[test]
#[should_panic(
    expected: (
        "Descriptor: `selector` mismatch, expected `131865267188622158278053964160834676621529874568090955194814616371745985007` but found `3123252206139358744730647958636922105676576163624049771737508399526017186883`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_attacker_control_hashes_model_registration() {
    let owner = starknet::contract_address_const::<'owner'>();
    let attacker = starknet::contract_address_const::<'attacker'>();

    starknet::testing::set_account_contract_address(owner);
    starknet::testing::set_contract_address(owner);

    // Owner deploys the world and register Foo model.
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo_selector = Model::<Foo>::selector();

    assert(world.is_owner(foo_selector, owner), 'should be owner');

    starknet::testing::set_contract_address(attacker);
    starknet::testing::set_account_contract_address(attacker);

    // Attacker has control over the this namespace.
    world.register_namespace("atk");

    // Attacker can't take ownership of the Foo model.
    world.register_model(attacker_model::TEST_CLASS_HASH.try_into().unwrap());
}

/// Test that an attacker can't control the hashes of resources in other namespaces
/// by deploying a contract in an other namespace.
#[test]
#[should_panic(
    expected: (
        "Descriptor: `selector` mismatch, expected `2256968028355087182573300510211413559640627226911800172611266486245255986230` but found `3123252206139358744730647958636922105676576163624049771737508399526017186883`",
        'ENTRYPOINT_FAILED',
    )
)]
fn test_attacker_control_hashes_contract_deployment() {
    let owner = starknet::contract_address_const::<'owner'>();
    let attacker = starknet::contract_address_const::<'attacker'>();

    starknet::testing::set_account_contract_address(owner);
    starknet::testing::set_contract_address(owner);

    // Owner deploys the world and register Foo model.
    let world = deploy_world();
    world.register_model(foo::TEST_CLASS_HASH.try_into().unwrap());

    let foo_selector = Model::<Foo>::selector();

    assert(world.is_owner(foo_selector, owner), 'should be owner');

    starknet::testing::set_contract_address(attacker);
    starknet::testing::set_account_contract_address(attacker);

    // Attacker has control over the this namespace.
    world.register_namespace("atk");

    // Attacker can't take ownership of the Foo model.
    world.register_contract('salt1', attacker_contract::TEST_CLASS_HASH.try_into().unwrap());
}

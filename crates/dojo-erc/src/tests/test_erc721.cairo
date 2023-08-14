use core::zeroable::Zeroable;
use core::traits::{Into, Default};
use array::ArrayTrait;
use serde::Serde;
use starknet::ContractAddress;
use starknet::testing::set_contract_address;

use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo_erc::tests::test_erc721_utils::{spawn_world, deploy_erc721, deploy_default};
use dojo_erc::erc721::interface::{IERC721, IERC721Dispatcher, IERC721DispatcherTrait};


// !!! u256 are "truncted" as felt252
// actually it's possible to mint -> burn -> mint -> ...
// todo : add Minted component to keep track of minted ids

fn deployer() -> ContractAddress {
    starknet::contract_address_const::<0x420>()
}

fn user1() -> ContractAddress {
    starknet::contract_address_const::<0x111>()
}

fn user2() -> ContractAddress {
    starknet::contract_address_const::<0x222>()
}


#[test]
#[available_gas(30000000)]
fn test_deploy() {
    let world = spawn_world();
    let erc721_address = deploy_erc721(world, deployer(), 'name', 'symbol', 'uri', 'seed-42');
    let erc721 = IERC721Dispatcher { contract_address: erc721_address };

    assert(erc721.owner() == deployer(), 'invalid owner');
    assert(erc721.name() == 'name', 'invalid name');
    assert(erc721.symbol() == 'symbol', 'invalid symbol');
    assert(erc721.token_uri(0) == 'uri', 'invalid uri')
}


#[test]
#[available_gas(30000000)]
fn test_deploy_default() {
    let (world, erc721) = deploy_default();
    assert(erc721.name() == 'name', 'invalid name');
}


#[test]
#[available_gas(30000000)]
fn test_mint_simple() {
    let (world, erc721) = deploy_default();

    erc721.mint(user1(), 0);
    erc721.mint(user2(), 1);
    erc721.mint(user2(), 2);

    assert(erc721.balance_of(user1()) == 1, 'balance should be 1');
    assert(erc721.balance_of(user2()) == 2, 'balance should be 2');

    assert(erc721.owner_of(0) == user1(), 'invalid owner');
    assert(erc721.owner_of(1) == user2(), 'invalid owner');
    assert(erc721.owner_of(2) == user2(), 'invalid owner');
}

#[test]
#[available_gas(30000000)]
#[should_panic(
    expected: (
        'ERC721: already minted',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED'
    )
)]
fn test_mint_same_id_twice() {
    let (world, erc721) = deploy_default();

    erc721.mint(user1(), 0);
    erc721.mint(user2(), 0); // should panic
}

#[test]
#[available_gas(30000000)]
fn test_burn_simple() {
    let (world, erc721) = deploy_default();

    erc721.mint(user1(), 42);

    assert(erc721.balance_of(user1()) == 1, 'balance should be 1');
    assert(erc721.owner_of(42) == user1(), 'invalid owner');

    // impersonate user1
    set_contract_address(user1());
    erc721.burn(42);

    assert(erc721.balance_of(user1()) == 0, 'balance should be 0');
    assert(erc721.owner_of(42).is_zero(), 'invalid owner');
}

#[test]
#[available_gas(30000000)]
fn test_burn_from_approved() {
    let (world, erc721) = deploy_default();

    erc721.mint(user1(), 42);

    assert(erc721.balance_of(user1()) == 1, 'balance should be 1');
    assert(erc721.owner_of(42) == user1(), 'invalid owner');

    // impersonate user1
    set_contract_address(user1());

    // user1 approve user2 for token 42
    erc721.approve(user2(), 42);

    // impersonate user2
    set_contract_address(user2());
    erc721.burn(42);

    assert(erc721.balance_of(user1()) == 0, 'balance should be 0');
    assert(erc721.owner_of(42).is_zero(), 'invalid owner');
}


#[test]
#[available_gas(30000000)]
fn test_burn_from_approved_operator() {
    let (world, erc721) = deploy_default();

    erc721.mint(user1(), 42);

    assert(erc721.balance_of(user1()) == 1, 'balance should be 1');
    assert(erc721.owner_of(42) == user1(), 'invalid owner');

    // impersonate user1
    set_contract_address(user1());

    // user1 set_approval_for_all to user2
    erc721.set_approval_for_all(user2(), true);

    // impersonate user2
    set_contract_address(user2());

    //user2 burn user1 token 42
    erc721.burn(42);
    assert(erc721.owner_of(42).is_zero(), 'should be 0');
}

#[test]
#[available_gas(30000000)]
#[should_panic(
    expected: (
        'ERC721: invalid token_id',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED'
    )
)]
fn test_burn_invalid_id() {
    let (world, erc721) = deploy_default();
    erc721.burn(0); // should panic
}

#[test]
#[available_gas(30000000)]
#[should_panic(
    expected: (
        'ERC721: unauthorized caller',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED'
    )
)]
fn test_burn_not_owned_id() {
    let (world, erc721) = deploy_default();
    erc721.mint(user1(), 0);

    // impersonate deployer
    set_contract_address(deployer());

    erc721.burn(0); // should panic
}


#[test]
#[available_gas(30000000)]
fn test_approve() {
    let (world, erc721) = deploy_default();

    erc721.mint(user1(), 0);
    assert(erc721.get_approved(0).is_zero(), 'should be 0');

    // impersonate user1
    set_contract_address(user1());

    // approve user2 for token 0
    erc721.approve(user2(), 0);
    assert(erc721.get_approved(0) == user2(), 'should be user2 address');

    // impersonate user2
    set_contract_address(user2());
    // user2 can transfer_from token 0
    erc721.transfer_from(user1(), user2(), 0);
    assert(erc721.owner_of(0) == user2(), 'owner should be user2');

    // approval is reset after transfer
    assert(erc721.get_approved(0).is_zero(), 'shoud reset to 0');
}

#[test]
#[available_gas(50000000)]
fn test_set_approval_for_all() {
    let (world, erc721) = deploy_default();

    erc721.mint(user1(), 0);
    erc721.mint(user1(), 1);

    // impersonate user1
    set_contract_address(user1());

    // approve user2 
    erc721.set_approval_for_all(user2(), true);
    assert(erc721.is_approved_for_all(user1(), user2()), 'should be approved');

    // impersonate user2
    set_contract_address(user2());

    // user2 can approve
    erc721.approve(user2(), 0);
    erc721.approve(user2(), 1);
    assert(erc721.get_approved(0) == user2(), 'should be user2 address');
    assert(erc721.get_approved(1) == user2(), 'should be user2 address');
    // user2 can transfer_from
    erc721.transfer_from(erc721.owner_of(0), user2(), 0);
    erc721.transfer_from(erc721.owner_of(0), user2(), 1);
    assert(erc721.owner_of(0) == user2(), 'should be user2 address');
    assert(erc721.owner_of(1) == user2(), 'should be user2 address');
}


#[test]
#[available_gas(50000000)]
fn test_transfer() {
    let (world, erc721) = deploy_default();

    erc721.mint(user1(), 0);

    // impersonate user1
    set_contract_address(user1());

    erc721.transfer(user2(), 0);
    assert(erc721.owner_of(0) == user2(), 'should be user2 address');
}

#[test]
#[available_gas(50000000)]
fn test_transfer_reset_approvals() {
    let (world, erc721) = deploy_default();

    erc721.mint(user1(), 0);

    // impersonate user1
    set_contract_address(user1());

    // approve deployer for token 0
    erc721.approve(deployer(), 0);

    erc721.transfer(user2(), 0);
    assert(erc721.owner_of(0) == user2(), 'should be user2 address');
    assert(erc721.get_approved(0).is_zero(), 'should reset approval');
}


#[test]
#[available_gas(50000000)]
fn test_transfer_from_approved() {
    let (world, erc721) = deploy_default();

    erc721.mint(user1(), 0);

    // impersonate user1
    set_contract_address(user1());

    // user1 approve user2
    erc721.approve(user2(), 0);

    // impersonate user2
    set_contract_address(user2());

    // user2 can transfer_from
    erc721.transfer_from(user1(), user2(), 0);
    assert(erc721.owner_of(0) == user2(), 'should be user2 address');
}

#[test]
#[available_gas(50000000)]
fn test_transfer_from_approved_operator() {
    let (world, erc721) = deploy_default();

    erc721.mint(user1(), 0);

    // impersonate user1
    set_contract_address(user1());

    // user1 set_approve_for_all user2
    erc721.set_approval_for_all(user2(), true);

    // impersonate user2
    set_contract_address(user2());

    // user2 can transfer_from
    erc721.transfer_from(user1(), user2(), 0);
    assert(erc721.owner_of(0) == user2(), 'should be user2 address');
}
// TODO: safe_transfer_from
// TODO: more tests



use core::zeroable::Zeroable;
use core::traits::{Into, Default};
use array::ArrayTrait;
use serde::Serde;
use starknet::ContractAddress;
use starknet::testing::set_contract_address;
use option::OptionTrait;

use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo_erc::tests::test_erc721_utils::{
    spawn_world, deploy_erc721, deploy_default, USER1, USER2, DEPLOYER, ZERO, PROXY
};
use dojo_erc::erc721::interface::{IERC721, IERC721Dispatcher, IERC721DispatcherTrait};
use dojo_erc::erc721::erc721::ERC721::{Event, Transfer, Approval, ApprovalForAll};
// actually it's possible to mint -> burn -> mint -> ...
// todo : add Minted component to keep track of minted ids

#[test]
#[available_gas(30000000)]
fn test_deploy() {
    let world = spawn_world();
    let erc721_address = deploy_erc721(world, DEPLOYER(), 'name', 'symbol', 'uri', 'seed-42');
    let erc721 = IERC721Dispatcher { contract_address: erc721_address };

    assert(erc721.owner() == DEPLOYER(), 'invalid owner');
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

    erc721.mint(USER1(), 0);
    erc721.mint(USER2(), 1);
    erc721.mint(USER2(), 2);

    assert(erc721.balance_of(USER1()) == 1, 'balance should be 1');
    assert(erc721.balance_of(USER2()) == 2, 'balance should be 2');

    assert(erc721.owner_of(0) == USER1(), 'invalid owner');
    assert(erc721.owner_of(1) == USER2(), 'invalid owner');
    assert(erc721.owner_of(2) == USER2(), 'invalid owner');
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

    erc721.mint(USER1(), 0);
    erc721.mint(USER2(), 0); // should panic
}

#[test]
#[available_gas(30000000)]
fn test_burn_simple() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 42);

    assert(erc721.balance_of(USER1()) == 1, 'balance should be 1');
    assert(erc721.owner_of(42) == USER1(), 'invalid owner');

    // impersonate user1
    set_contract_address(USER1());
    erc721.burn(42);

    assert(erc721.balance_of(USER1()) == 0, 'balance should be 0');
    assert(erc721.owner_of(42).is_zero(), 'invalid owner');
}

#[test]
#[available_gas(30000000)]
fn test_burn_from_approved() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 42);

    assert(erc721.balance_of(USER1()) == 1, 'balance should be 1');
    assert(erc721.owner_of(42) == USER1(), 'invalid owner');

    // impersonate user1
    set_contract_address(USER1());

    // user1 approve user2 for token 42
    erc721.approve(USER2(), 42);

    // impersonate user2
    set_contract_address(USER2());
    erc721.burn(42);

    assert(erc721.balance_of(USER1()) == 0, 'balance should be 0');
    assert(erc721.owner_of(42).is_zero(), 'invalid owner');
}


#[test]
#[available_gas(30000000)]
fn test_burn_from_approved_operator() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 42);

    assert(erc721.balance_of(USER1()) == 1, 'balance should be 1');
    assert(erc721.owner_of(42) == USER1(), 'invalid owner');

    // impersonate user1
    set_contract_address(USER1());

    // user1 set_approval_for_all to user2
    erc721.set_approval_for_all(USER2(), true);

    // impersonate user2
    set_contract_address(USER2());

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
    erc721.mint(USER1(), 0);

    // impersonate deployer
    set_contract_address(DEPLOYER());

    erc721.burn(0); // should panic
}


#[test]
#[available_gas(30000000)]
fn test_approve() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 0);
    assert(erc721.get_approved(0).is_zero(), 'should be 0');

    // impersonate user1
    set_contract_address(USER1());

    // approve user2 for token 0
    erc721.approve(USER2(), 0);
    assert(erc721.get_approved(0) == USER2(), 'should be user2 address');

    // impersonate user2
    set_contract_address(USER2());
    // user2 can transfer_from token 0
    erc721.transfer_from(USER1(), USER2(), 0);
    assert(erc721.owner_of(0) == USER2(), 'owner should be user2');

    // approval is reset after transfer
    assert(erc721.get_approved(0).is_zero(), 'shoud reset to 0');
}

#[test]
#[available_gas(50000000)]
fn test_set_approval_for_all() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 0);
    erc721.mint(USER1(), 1);

    // impersonate user1
    set_contract_address(USER1());

    // approve user2 
    erc721.set_approval_for_all(USER2(), true);
    assert(erc721.is_approved_for_all(USER1(), USER2()), 'should be approved');

    // impersonate user2
    set_contract_address(USER2());

    // user2 can approve
    erc721.approve(USER2(), 0);
    erc721.approve(USER2(), 1);
    assert(erc721.get_approved(0) == USER2(), 'should be user2 address');
    assert(erc721.get_approved(1) == USER2(), 'should be user2 address');
    // user2 can transfer_from
    erc721.transfer_from(erc721.owner_of(0), USER2(), 0);
    erc721.transfer_from(erc721.owner_of(0), USER2(), 1);
    assert(erc721.owner_of(0) == USER2(), 'should be user2 address');
    assert(erc721.owner_of(1) == USER2(), 'should be user2 address');
}


#[test]
#[available_gas(50000000)]
fn test_transfer() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 0);

    // impersonate user1
    set_contract_address(USER1());

    erc721.transfer(USER2(), 0);
    assert(erc721.owner_of(0) == USER2(), 'should be user2 address');
}

#[test]
#[available_gas(50000000)]
fn test_transfer_reset_approvals() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 0);

    // impersonate user1
    set_contract_address(USER1());

    // approve deployer for token 0
    erc721.approve(DEPLOYER(), 0);

    erc721.transfer(USER2(), 0);
    assert(erc721.owner_of(0) == USER2(), 'should be user2 address');
    assert(erc721.get_approved(0).is_zero(), 'should reset approval');
}


#[test]
#[available_gas(50000000)]
fn test_transfer_from_approved() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 0);

    // impersonate user1
    set_contract_address(USER1());

    // user1 approve user2
    erc721.approve(USER2(), 0);

    // impersonate user2
    set_contract_address(USER2());

    // user2 can transfer_from
    erc721.transfer_from(USER1(), USER2(), 0);
    assert(erc721.owner_of(0) == USER2(), 'should be user2 address');
}

#[test]
#[available_gas(50000000)]
fn test_transfer_from_approved_operator() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 0);

    // impersonate user1
    set_contract_address(USER1());

    // user1 set_approve_for_all user2
    erc721.set_approval_for_all(USER2(), true);

    // impersonate user2
    set_contract_address(USER2());

    // user2 can transfer_from
    erc721.transfer_from(USER1(), USER2(), 0);
    assert(erc721.owner_of(0) == USER2(), 'should be user2 address');
}


//
// Events
//
use debug::PrintTrait;

#[test]
#[available_gas(50000000)]
fn test_event_transfer() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 42);
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::Transfer(Transfer { from: ZERO(), to: USER1(), token_id: 42 }),
        'invalid Transfer event'
    );

    // impersonate user1
    set_contract_address(USER1());
    // transfer token_id 42 from user1 to user2
    erc721.transfer(USER2(), 42);
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::Transfer(Transfer { from: USER1(), to: USER2(), token_id: 42 }),
        'invalid Transfer event'
    );
// ERROR : 
// error: Failed setting up runner.
// Caused by:
// #24625->#24626: Got 'Unknown ap change' error while moving [3].

// impersonate user2
//set_contract_address(USER2());
// // user2 burns token_id 42
// erc721.burn(42);
// assert(
//     @starknet::testing::pop_log(erc721.contract_address)
//         .unwrap() == @Event::Transfer(Transfer { from: USER2(), to: ZERO(), token_id: 42 }),
//     'invalid Transfer event'
// );
}


#[test]
#[available_gas(50000000)]
fn test_event_approval() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 42);
    let _: Event = starknet::testing::pop_log(erc721.contract_address).unwrap();

    // impersonate user1
    set_contract_address(USER1());

    erc721.approve(PROXY(), 42);
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::Approval(Approval { owner: USER1(), to: PROXY(), token_id: 42 }),
        'invalid Approval event'
    );
}

#[test]
#[available_gas(50000000)]
fn test_event_approval_for_all() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 42);
    let _: Event = starknet::testing::pop_log(erc721.contract_address).unwrap();

    // impersonate user1
    set_contract_address(USER1());

    erc721.set_approval_for_all(PROXY(), true);
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::ApprovalForAll(
                ApprovalForAll { owner: USER1(), operator: PROXY(), approved: true }
            ),
        'invalid ApprovalForAll event'
    );
}
// TODO: more tests 
// TODO: check invalid approval & approval_for_all



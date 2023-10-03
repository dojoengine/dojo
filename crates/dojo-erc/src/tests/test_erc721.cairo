use core::zeroable::Zeroable;
use core::traits::{Into, Default};
use array::ArrayTrait;
use serde::Serde;
use starknet::ContractAddress;
use starknet::testing::set_contract_address;
use option::OptionTrait;

use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo_erc::tests::test_utils::impersonate;
use dojo_erc::tests::test_erc721_utils::{
    spawn_world, deploy_erc721, deploy_default, deploy_testcase1, USER1, USER2, USER3, DEPLOYER,
    ZERO, PROXY
};


use dojo_erc::erc165::interface::IERC165_ID;
use dojo_erc::erc721::interface::{
    IERC721, IERC721ADispatcher, IERC721ADispatcherTrait, IERC721_ID, IERC721_METADATA_ID
};
use dojo_erc::erc721::erc721::ERC721::{Event, Transfer, Approval, ApprovalForAll};
// actually it's possible to mint -> burn -> mint -> ...
// todo : add Minted component to keep track of minted ids

#[test]
#[available_gas(30000000)]
fn test_deploy() {
    let world = spawn_world(DEPLOYER());
    let erc721_address = deploy_erc721(world, DEPLOYER(), 'name', 'symbol', 'uri', 'seed-42');
    let erc721 = IERC721ADispatcher { contract_address: erc721_address };

    assert(erc721.owner() == DEPLOYER(), 'invalid owner');
    assert(erc721.name() == 'name', 'invalid name');
    assert(erc721.symbol() == 'symbol', 'invalid symbol');
}


#[test]
#[available_gas(30000000)]
fn test_deploy_default() {
    let (world, erc721) = deploy_default();
    assert(erc721.name() == 'name', 'invalid name');
}

//
// supports_interface
//

#[test]
#[available_gas(30000000)]
fn test_should_support_interfaces() {
    let (world, erc721) = deploy_default();

    assert(erc721.supports_interface(IERC165_ID) == true, 'should support erc165');
    assert(erc721.supports_interface(IERC721_ID) == true, 'should support erc721');
    assert(
        erc721.supports_interface(IERC721_METADATA_ID) == true, 'should support erc721_metadata'
    );
}


//
// behaves like an ERC721
//

//
// balance_of
//

use debug::PrintTrait;

#[test]
#[available_gas(60000000)]
fn test_balance_of_with_tokens() {
    // returns the amount of tokens owned by the given address

    let (world, erc721) = deploy_testcase1();
    assert(erc721.balance_of(USER1()) == 3, 'should be 3');
    assert(erc721.balance_of(PROXY()) == 4, 'should be 4');
}

#[test]
#[available_gas(60000000)]
fn test_balance_of_with_no_tokens() {
    // when the given address does not own any tokens

    let (world, erc721) = deploy_testcase1();
    assert(erc721.balance_of(USER3()) == 0, 'should be 0');
}


#[test]
#[available_gas(50000000)]
#[should_panic]
fn test_balance_of_zero_address() {
    // when querying the zero address

    let (world, erc721) = deploy_testcase1();
    erc721.balance_of(ZERO());
}

//
// owner_of
//

#[test]
#[available_gas(90000000)]
fn test_owner_of_existing_id() {
    // when the given token ID was tracked by this token = for existing id

    let (world, erc721) = deploy_testcase1();
    assert(erc721.owner_of(1) == USER1(), 'should be user1');
    assert(erc721.owner_of(2) == USER1(), 'should be user1');
    assert(erc721.owner_of(3) == USER1(), 'should be user1');

    assert(erc721.owner_of(10) == PROXY(), 'should be proxy');
    assert(erc721.owner_of(11) == PROXY(), 'should be proxy');
    assert(erc721.owner_of(12) == PROXY(), 'should be proxy');
    assert(erc721.owner_of(13) == PROXY(), 'should be proxy');
}


#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_owner_of_non_existing_id() {
    // when the given token ID was not tracked by this token = non existing id

    let (world, erc721) = deploy_testcase1();
    let owner_of_0 = erc721.owner_of(0); // should panic
}

//
// transfers
//

#[test]
#[available_gas(90000000)]
fn test_transfer_ownership() {
    // transfers the ownership of the given token ID to the given address

    let (world, erc721) = deploy_testcase1();

    impersonate(USER1());

    let owner_of_1 = erc721.owner_of(1);
    // transfer token_id 1 to user2
    erc721.transfer(USER2(), 1);
    assert(erc721.owner_of(1) == USER2(), 'invalid owner');
}

#[test]
#[available_gas(90000000)]
fn test_transfer_event() {
    // emits a Transfer event

    let (world, erc721) = deploy_default();

    // mint
    erc721.mint(USER1(), 42);

    impersonate(USER1());

    // transfer token_id 1 to user2
    erc721.transfer(USER2(), 42);

    impersonate(USER2());
    erc721.burn(42);

    // mint
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::Transfer(Transfer { from: ZERO(), to: USER1(), token_id: 42 }),
        'invalid Transfer event'
    );
    // transfer
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::Transfer(Transfer { from: USER1(), to: USER2(), token_id: 42 }),
        'invalid Transfer event'
    );
    // burn
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::Transfer(Transfer { from: USER2(), to: ZERO(), token_id: 42 }),
        'invalid Transfer event'
    );
}


#[test]
#[available_gas(90000000)]
fn test_transfer_clear_approval() {
    // clears the approval for the token ID

    let (world, erc721) = deploy_testcase1();

    impersonate(USER1());

    erc721.approve(PROXY(), 1);
    assert(erc721.get_approved(1) == PROXY(), 'should be proxy');

    // transfer token_id 1 to user2
    erc721.transfer(USER2(), 1);
    assert(erc721.get_approved(1).is_zero(), 'should be zero');
}

#[test]
#[available_gas(90000000)]
fn test_transfer_adjusts_owners_balances() {
    // adjusts owners balances

    let (world, erc721) = deploy_testcase1();

    impersonate(USER1());

    let balance_user1_before = erc721.balance_of(USER1());
    let balance_user2_before = erc721.balance_of(USER2());

    // transfer token_id 1 to user2
    erc721.transfer(USER2(), 1);

    let balance_user1_after = erc721.balance_of(USER1());
    let balance_user2_after = erc721.balance_of(USER2());

    assert(balance_user1_after == balance_user1_before - 1, 'invalid user1 balance');
    assert(balance_user2_after == balance_user2_before + 1, 'invalid user2 balance');
}


#[test]
#[available_gas(90000000)]
fn test_transfer_from_approved() {
    // when called by the approved individual

    let (world, erc721) = deploy_testcase1();

    impersonate(USER1());

    //user1 approve user2 for token_id 2
    erc721.approve(USER2(), 2);

    impersonate(USER2());

    erc721.transfer_from(USER1(), USER2(), 2);
    assert(erc721.owner_of(2) == USER2(), 'invalid owner');
}

#[test]
#[available_gas(90000000)]
fn test_transfer_from_approved_operator() {
    // when called by the operator

    let (world, erc721) = deploy_testcase1();

    impersonate(USER1());

    //user1 set_approval_for_all for proxy
    erc721.set_approval_for_all(PROXY(), true);

    impersonate(PROXY());

    erc721.transfer_from(USER1(), USER2(), 2);
    assert(erc721.owner_of(2) == USER2(), 'invalid owner');
}

#[test]
#[available_gas(90000000)]
fn test_transfer_from_owner_without_approved() {
    // when called by the owner without an approved user

    let (world, erc721) = deploy_testcase1();

    impersonate(USER1());

    erc721.approve(ZERO(), 2);

    erc721.transfer_from(USER1(), USER2(), 2);
    assert(erc721.owner_of(2) == USER2(), 'invalid owner');
}


#[test]
#[available_gas(90000000)]
fn test_transfer_to_owner() {
    // when sent to the owner

    let (world, erc721) = deploy_testcase1();

    impersonate(USER1());

    let balance_before = erc721.balance_of(USER1());

    assert(erc721.owner_of(3) == USER1(), 'invalid owner');
    erc721.transfer(USER1(), 3);

    // keeps ownership of the token
    assert(erc721.owner_of(3) == USER1(), 'invalid owner');

    // clears the approval for the token ID
    assert(erc721.get_approved(3) == ZERO(), 'invalid approved');

    //emits only a transfer event  : cumbersome to test with pop_log

    //keeps the owner balance
    let balance_after = erc721.balance_of(USER1());
    assert(balance_before == balance_after, 'invalid balance')
}


#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_transfer_when_previous_owner_is_incorrect() {
    // when the address of the previous owner is incorrect

    let (world, erc721) = deploy_testcase1();

    impersonate(USER1());

    //user2 owner token_id 10
    erc721.transfer_from(USER1(), PROXY(), 10); // should panic
}

#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_transfer_when_sender_not_authorized() {
    // when the sender is not authorized for the token id
    let (world, erc721) = deploy_testcase1();

    impersonate(PROXY());

    //proxy is not authorized for USER2
    erc721.transfer_from(USER2(), PROXY(), 20); // should panic
}

#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_transfer_when_token_id_doesnt_exists() {
    // when the sender is not authorized for the token id
    let (world, erc721) = deploy_testcase1();

    impersonate(PROXY());

    //proxy is  authorized for USER1 but token_id 50 doesnt exists
    erc721.transfer_from(USER1(), PROXY(), 50); // should panic
}


#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_transfer_to_address_zero() {
    // when the address to transfer the token to is the zero address
    let (world, erc721) = deploy_testcase1();

    impersonate(USER1());

    erc721.transfer(ZERO(), 1); // should panic
}

//
// approval
//

// when clearing approval

#[test]
#[available_gas(90000000)]
fn test_approval_when_clearing_with_prior_approval() {
    // -when there was a prior approval
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 42);

    impersonate(USER1());

    erc721.approve(PROXY(), 42);

    //revoke approve
    erc721.approve(ZERO(), 42);

    // clears approval for the token               
    assert(erc721.get_approved(42) == ZERO(), 'invalid approved');

    // emits an approval event
    let _: Event = starknet::testing::pop_log(erc721.contract_address).unwrap(); // unpop mint
    let _: Event = starknet::testing::pop_log(erc721.contract_address)
        .unwrap(); // unpop approve PROXY

    // approve ZERO
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::Approval(Approval { owner: USER1(), to: ZERO(), token_id: 42 }),
        'invalid Approval event'
    );
}

#[test]
#[available_gas(90000000)]
fn test_approval_when_clearing_without_prior_approval() {
    // when clearing approval
    // -when there was no prior approval
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 42);

    impersonate(USER1());

    //revoke approve
    erc721.approve(ZERO(), 42);

    // updates approval for the token               
    assert(erc721.get_approved(42) == ZERO(), 'invalid approved');

    let _: Event = starknet::testing::pop_log(erc721.contract_address).unwrap(); // unpop mint

    // approve ZERO
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::Approval(Approval { owner: USER1(), to: ZERO(), token_id: 42 }),
        'invalid Approval event'
    );
}


// when approving a non-zero address

#[test]
#[available_gas(90000000)]
fn test_approval_non_zero_address_with_prior_approval() {
    // -when there was a prior approval
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 42);

    impersonate(USER1());
    erc721.approve(PROXY(), 42);

    // user1 approves user3
    erc721.approve(USER3(), 42);

    // set approval for the token               
    assert(erc721.get_approved(42) == USER3(), 'invalid approved');

    // emits an approval event
    let _: Event = starknet::testing::pop_log(erc721.contract_address).unwrap(); // unpop mint
    let _: Event = starknet::testing::pop_log(erc721.contract_address)
        .unwrap(); // unpop approve PROXY

    // approve USER3
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::Approval(Approval { owner: USER1(), to: USER3(), token_id: 42 }),
        'invalid Approval event'
    );
}

#[test]
#[available_gas(90000000)]
fn test_approval_non_zero_address_with_no_prior_approval() {
    // -when there was no prior approval
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 42);

    impersonate(USER1());

    // user1 approves user3
    erc721.approve(USER3(), 42);

    // set approval for the token               
    assert(erc721.get_approved(42) == USER3(), 'invalid approved');

    // emits an approval event
    let _: Event = starknet::testing::pop_log(erc721.contract_address).unwrap(); // unpop mint

    // approve USER3
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::Approval(Approval { owner: USER1(), to: USER3(), token_id: 42 }),
        'invalid Approval event'
    );
}


#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_approval_self_approve() {
    // when the address that receives the approval is the owner
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 42);

    impersonate(USER1());

    // user1 approves user1
    erc721.approve(USER1(), 42); // should panic
}

#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_approval_not_owned() {
    // when the sender does not own the given token ID

    let (world, erc721) = deploy_testcase1();

    impersonate(USER1());

    // user1 approves user2 for token 20
    erc721.approve(USER2(), 20); // should panic
}


#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_approval_from_approved_sender() {
    // when the sender is approved for the given token ID

    let (world, erc721) = deploy_testcase1();

    impersonate(USER1());

    // user1 approve user3
    erc721.approve(USER3(), 1);

    impersonate(USER3());

    // (ERC721: approve caller is not token owner or approved for all)
    erc721.approve(USER2(), 1); // should panic
}


#[test]
#[available_gas(90000000)]
fn test_approval_from_approved_operator() {
    // when the sender is an operator
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 50);

    impersonate(USER1());

    erc721.set_approval_for_all(PROXY(), true);

    impersonate(PROXY());

    // proxy approves user2 for token 20
    erc721.approve(USER2(), 50);

    assert(erc721.get_approved(50) == USER2(), 'invalid approval');

    let _: Event = starknet::testing::pop_log(erc721.contract_address).unwrap(); // unpop mint
    let _: Event = starknet::testing::pop_log(erc721.contract_address)
        .unwrap(); // unpop set_approval_for_all

    // approve 
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::Approval(Approval { owner: USER1(), to: USER2(), token_id: 50 }),
        'invalid Approval event'
    );
}


#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_approval_unexisting_id() {
    // when the given token ID does not exist
    let (world, erc721) = deploy_testcase1();

    impersonate(USER1());

    // user1 approve user3
    erc721.approve(USER3(), 69); // should panic
}

//
// approval_for_all
//

#[test]
#[available_gas(90000000)]
fn test_approval_for_all_operator_is_not_owner_no_operator_approval() {
    // when the operator willing to approve is not the owner
    // -when there is no operator approval set by the sender
    let (world, erc721) = deploy_default();

    impersonate(USER2());

    // user2 set_approval_for_all PROXY
    erc721.set_approval_for_all(PROXY(), true);

    assert(erc721.is_approved_for_all(USER2(), PROXY()) == true, 'invalid is_approved_for_all');

    // ApproveForAll 
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::ApprovalForAll(
                ApprovalForAll { owner: USER2(), operator: PROXY(), approved: true }
            ),
        'invalid ApprovalForAll event'
    );
}

#[test]
#[available_gas(90000000)]
fn test_approval_for_all_operator_is_not_owner_from_not_approved() {
    // when the operator willing to approve is not the owner
    // -when the operator was set as not approved
    let (world, erc721) = deploy_default();

    impersonate(USER2());

    erc721.set_approval_for_all(PROXY(), false);

    // user2 set_approval_for_all PROXY
    erc721.set_approval_for_all(PROXY(), true);

    assert(erc721.is_approved_for_all(USER2(), PROXY()) == true, 'invalid is_approved_for_all');

    let _: Event = starknet::testing::pop_log(erc721.contract_address)
        .unwrap(); // unpop set_approval_for_all(PROXY(), false)

    // ApproveForAll 
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::ApprovalForAll(
                ApprovalForAll { owner: USER2(), operator: PROXY(), approved: true }
            ),
        'invalid ApprovalForAll event'
    );
}

#[test]
#[available_gas(90000000)]
fn test_approval_for_all_operator_is_not_owner_can_unset_approval_for_all() {
    // when the operator willing to approve is not the owner
    // can unset the operator approval
    let (world, erc721) = deploy_default();

    impersonate(USER2());

    erc721.set_approval_for_all(PROXY(), false);
    erc721.set_approval_for_all(PROXY(), true);
    assert(erc721.is_approved_for_all(USER2(), PROXY()) == true, 'invalid is_approved_for_all');
    erc721.set_approval_for_all(PROXY(), false);
    assert(erc721.is_approved_for_all(USER2(), PROXY()) == false, 'invalid is_approved_for_all');

    let _: Event = starknet::testing::pop_log(erc721.contract_address)
        .unwrap(); // unpop set_approval_for_all(PROXY(), false)
    let _: Event = starknet::testing::pop_log(erc721.contract_address)
        .unwrap(); // unpop set_approval_for_all(PROXY(), true)

    // ApproveForAll 
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::ApprovalForAll(
                ApprovalForAll { owner: USER2(), operator: PROXY(), approved: false }
            ),
        'invalid ApprovalForAll event'
    );
}

#[test]
#[available_gas(90000000)]
fn test_approval_for_all_operator_with_operator_already_approved() {
    // when the operator willing to approve is not the owner
    // when the operator was already approved
    let (world, erc721) = deploy_default();

    impersonate(USER2());

    erc721.set_approval_for_all(PROXY(), true);
    assert(erc721.is_approved_for_all(USER2(), PROXY()) == true, 'invalid is_approved_for_all');
    erc721.set_approval_for_all(PROXY(), true);
    assert(erc721.is_approved_for_all(USER2(), PROXY()) == true, 'invalid is_approved_for_all');

    let _: Event = starknet::testing::pop_log(erc721.contract_address)
        .unwrap(); // unpop set_approval_for_all(PROXY(), true)

    // ApproveForAll 
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::ApprovalForAll(
                ApprovalForAll { owner: USER2(), operator: PROXY(), approved: true }
            ),
        'invalid ApprovalForAll event'
    );
}


#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_approval_for_all_with_owner_as_operator() {
    // when the operator is the owner

    let (world, erc721) = deploy_default();

    impersonate(USER1());

    erc721.set_approval_for_all(USER1(), true); // should panic
}


//
// get_approved
//

#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_get_approved_unexisting_token() {
    let (world, erc721) = deploy_default();

    erc721.get_approved(420); // should panic
}


#[test]
#[available_gas(90000000)]
fn test_get_approved_with_existing_token() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 420);
    assert(erc721.get_approved(420) == ZERO(), 'invalid get_approved');
}


#[test]
#[available_gas(90000000)]
fn test_get_approved_with_existing_token_and_approval() {
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 420);

    impersonate(USER1());

    erc721.approve(PROXY(), 420);
    assert(erc721.get_approved(420) == PROXY(), 'invalid get_approved');
}

//
// mint
//

#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_mint_to_address_zero() {
    // reverts with a null destination address

    let (world, erc721) = deploy_default();

    erc721.mint(ZERO(), 69); // should panic
}


#[test]
#[available_gas(90000000)]
fn test_mint() {
    // reverts with a null destination address

    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 69);

    assert(erc721.balance_of(USER1()) == 1, 'invalid balance');

    // Transfer 
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::Transfer(Transfer { from: ZERO(), to: USER1(), token_id: 69 }),
        'invalid Transfer event'
    );
}

#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_mint_existing_token_id() {
    // reverts with a null destination address

    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 69);
    erc721.mint(USER1(), 69); //should panic
}


//
// burn
//

#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_burn_non_existing_token_id() {
    //reverts when burning a non-existent token id
    let (world, erc721) = deploy_default();
    erc721.burn(69); // should panic
}


#[test]
#[available_gas(90000000)]
fn test_burn_emit_events() {
    // burn should emit event
    let (world, erc721) = deploy_default();

    erc721.mint(USER1(), 69);
    assert(erc721.balance_of(USER1()) == 1, 'invalid balance');

    impersonate(USER1());

    erc721.burn(69);
    assert(erc721.balance_of(USER1()) == 0, 'invalid balance');

    let _: Event = starknet::testing::pop_log(erc721.contract_address)
        .unwrap(); // unpop  erc721.mint(USER1(), 69)

    // Transfer 
    assert(
        @starknet::testing::pop_log(erc721.contract_address)
            .unwrap() == @Event::Transfer(Transfer { from: USER1(), to: ZERO(), token_id: 69 }),
        'invalid Transfer event'
    );
}


#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_burn_same_id_twice() {
    // reverts when burning a token id that has been deleted
    let (world, erc721) = deploy_default();
    erc721.mint(USER1(), 69);
    erc721.burn(69);
    erc721.burn(69); // should panic
}

//
// token_uri
//

#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_token_uri_for_non_existing_token_id() {
    // reverts when queried for non existent token id
    let (world, erc721) = deploy_default();
    erc721.token_uri(1234); // should panic
}


use zeroable::Zeroable;
use traits::{Into, Default, IndexView};
use option::OptionTrait;
use array::ArrayTrait;
use serde::Serde;
use starknet::ContractAddress;
use starknet::testing::set_contract_address;

use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo_erc::tests::test_utils::impersonate;
use dojo_erc::tests::test_erc1155_utils::{
    spawn_world, deploy_erc1155, deploy_default, deploy_testcase1, ZERO, USER1, USER2, DEPLOYER,
    PROXY
};

use dojo_erc::erc165::interface::IERC165_ID;
use dojo_erc::erc1155::interface::{
    IERC1155A, IERC1155ADispatcher, IERC1155ADispatcherTrait, IERC1155_ID, IERC1155_METADATA_ID,
    IERC1155_RECEIVER_ID
};

use dojo_erc::erc1155::erc1155::ERC1155::{Event, TransferSingle, TransferBatch, ApprovalForAll};


#[test]
#[available_gas(30000000)]
fn test_deploy() {
    let world = spawn_world(DEPLOYER());
    let erc1155_address = deploy_erc1155(world, DEPLOYER(), 'uri', 'seed-42');
    let erc1155 = IERC1155ADispatcher { contract_address: erc1155_address };
    assert(erc1155.owner() == DEPLOYER(), 'invalid owner');
}

#[test]
#[available_gas(30000000)]
fn test_deploy_default() {
    let (world, erc1155) = deploy_default();
    assert(erc1155.owner() == DEPLOYER(), 'invalid owner');
}


//
// supports_interface
//

#[test]
#[available_gas(30000000)]
fn test_should_support_interfaces() {
    let (world, erc1155) = deploy_default();

    assert(erc1155.supports_interface(IERC165_ID) == true, 'should support erc165');
    assert(erc1155.supports_interface(IERC1155_ID) == true, 'should support erc1155');
    assert(
        erc1155.supports_interface(IERC1155_METADATA_ID) == true, 'should support erc1155_metadata'
    );
}

//
// uri
//

#[test]
#[available_gas(30000000)]
fn test_uri() {
    let (world, erc1155) = deploy_default();
    assert(erc1155.uri(64) == 'uri', 'invalid uri');
}


//
// behaves like an ERC1155
//

//
// balance_of
//
#[test]
#[available_gas(30000000)]
#[should_panic(expected: ('ERC1155: invalid owner address', 'ENTRYPOINT_FAILED',))]
fn test_balance_of_zero_address() {
    //reverts when queried about the zero address

    let (world, erc1155) = deploy_default();
    erc1155.balance_of(ZERO(), 0); // should panic
}

#[test]
#[available_gas(30000000)]
fn test_balance_of_empty_balance() {
    // when accounts don't own tokens
    // returns zero for given addresses
    let (world, erc1155) = deploy_default();
    assert(erc1155.balance_of(USER1(), 0) == 0, 'should be 0');
    assert(erc1155.balance_of(USER1(), 69) == 0, 'should be 0');
    assert(erc1155.balance_of(USER2(), 0) == 0, 'should be 0');
}

#[test]
#[available_gas(30000000)]
fn test_balance_with_tokens() {
    // when accounts own some tokens
    // returns the amount of tokens owned by the given addresses
    let (world, erc1155) = deploy_default();

    erc1155.mint(USER1(), 0, 1, array![]);
    erc1155.mint(USER1(), 69, 42, array![]);
    erc1155.mint(USER2(), 69, 5, array![]);

    assert(erc1155.balance_of(USER1(), 0) == 1, 'should be 1');
    assert(erc1155.balance_of(USER1(), 69) == 42, 'should be 42');
    assert(erc1155.balance_of(USER2(), 69) == 5, 'should be 5');
}

//
// balance_of_batch
//

#[test]
#[available_gas(30000000)]
#[should_panic(expected: ('ERC1155: invalid length', 'ENTRYPOINT_FAILED',))]
fn test_balance_of_batch_with_invalid_input() {
    // reverts when input arrays don't match up
    let (world, erc1155) = deploy_default();
    erc1155.balance_of_batch(array![USER1(), USER2()], array![0]);
    erc1155.balance_of_batch(array![USER1()], array![0, 1, 2]);
}

#[test]
#[available_gas(30000000)]
#[should_panic(expected: ('ERC1155: invalid owner address', 'ENTRYPOINT_FAILED',))]
fn test_balance_of_batch_address_zero() {
    // reverts when input arrays don't match up
    let (world, erc1155) = deploy_default();
    erc1155.balance_of_batch(array![USER1(), ZERO()], array![0, 1]);
}

#[test]
#[available_gas(30000000)]
fn test_balance_of_batch_empty_account() {
    // when accounts don't own tokens
    // returns zeros for each account
    let (world, erc1155) = deploy_default();
    let balances = erc1155.balance_of_batch(array![USER1(), USER1(), USER1()], array![0, 1, 5]);
    let bals = @balances;
    assert(balances.len() == 3, 'should be 3');
    assert(bals[0] == @0_u256, 'should be 0');
    assert(bals[1] == @0_u256, 'should be 0');
    assert(bals[2] == @0_u256, 'should be 0');
}

#[test]
#[available_gas(30000000)]
fn test_balance_of_batch_with_tokens() {
    // when accounts own some tokens
    // returns amounts owned by each account in order passed
    let (world, erc1155) = deploy_default();

    erc1155.mint(USER1(), 0, 1, array![]);
    erc1155.mint(USER1(), 69, 42, array![]);
    erc1155.mint(USER2(), 69, 2, array![]);

    let balances = erc1155.balance_of_batch(array![USER1(), USER1(), USER2()], array![0, 69, 69]);
    let bals = @balances;
    assert(balances.len() == 3, 'should be 3');
    assert(bals[0] == @1_u256, 'should be 1');
    assert(bals[1] == @42_u256, 'should be 42');
    assert(bals[2] == @2_u256, 'should be 2');
}

#[test]
#[available_gas(30000000)]
fn test_balance_of_batch_with_tokens_2() {
    // when accounts own some tokens
    // returns multiple times the balance of the same address when asked
    let (world, erc1155) = deploy_default();

    erc1155.mint(USER1(), 0, 1, array![]);
    erc1155.mint(USER2(), 69, 2, array![]);

    let balances = erc1155.balance_of_batch(array![USER1(), USER2(), USER1()], array![0, 69, 0]);
    let bals = @balances;
    assert(balances.len() == 3, 'should be 3');
    assert(bals[0] == @1_u256, 'should be 1');
    assert(bals[1] == @2_u256, 'should be 2');
    assert(bals[2] == @1_u256, 'should be 1');
}


//
// balance_of_batch
//

#[test]
#[available_gas(30000000)]
fn test_set_approval_for_all() {
    // sets approval status which can be queried via is_approved_for_all
    let (world, erc1155) = deploy_default();
    impersonate(USER1());

    erc1155.set_approval_for_all(PROXY(), true);
    assert(erc1155.is_approved_for_all(USER1(), PROXY()) == true, 'should be true');
}

#[test]
#[available_gas(30000000)]
fn test_set_approval_for_all_emit_event() {
    // set_approval_for_all emits ApprovalForAll event
    let (world, erc1155) = deploy_default();
    impersonate(USER1());

    erc1155.set_approval_for_all(PROXY(), true);

    // ApprovalForAll
    assert(
        @starknet::testing::pop_log(erc1155.contract_address)
            .unwrap() == @Event::ApprovalForAll(
                ApprovalForAll { owner: USER1(), operator: PROXY(), approved: true }
            ),
        'invalid ApprovalForAll event'
    );
}


#[test]
#[available_gas(30000000)]
fn test_set_unset_approval_for_all() {
    // sets approval status which can be queried via is_approved_for_all
    let (world, erc1155) = deploy_default();
    impersonate(USER1());

    erc1155.set_approval_for_all(PROXY(), true);
    assert(erc1155.is_approved_for_all(USER1(), PROXY()) == true, 'should be true');
    erc1155.set_approval_for_all(PROXY(), false);
    assert(erc1155.is_approved_for_all(USER1(), PROXY()) == false, 'should be false');
}

#[test]
#[available_gas(30000000)]
#[should_panic()]
fn test_set_approval_for_all_on_self() {
    // reverts if attempting to approve self as an operator
    let (world, erc1155) = deploy_default();
    impersonate(USER1());

    erc1155.set_approval_for_all(USER1(), true); // should panic
}

//
// safe_transfer_from
//

#[test]
#[available_gas(30000000)]
#[should_panic()]
fn test_safe_transfer_from_more_than_balance() {
    // reverts when transferring more than balance
    let (world, erc1155) = deploy_testcase1();

    impersonate(USER1());

    erc1155.safe_transfer_from(USER1(), USER2(), 1, 999, array![]); // should panic
}

#[test]
#[available_gas(30000000)]
#[should_panic()]
fn test_safe_transfer_to_zero() {
    // reverts when transferring to zero address
    let (world, erc1155) = deploy_testcase1();

    impersonate(USER1());

    erc1155.safe_transfer_from(USER1(), ZERO(), 1, 1, array![]); // should panic
}

#[test]
#[available_gas(50000000)]
fn test_safe_transfer_debit_sender() {
    // debits transferred balance from sender
    let (world, erc1155) = deploy_testcase1();

    impersonate(USER1());

    let balance_before = erc1155.balance_of(USER1(), 1);
    erc1155.safe_transfer_from(USER1(), USER2(), 1, 1, array![]);
    let balance_after = erc1155.balance_of(USER1(), 1);

    assert(balance_after == balance_before - 1, 'invalid balance after');
}

#[test]
#[available_gas(50000000)]
fn test_safe_transfer_credit_receiver() {
    // credits transferred balance to receiver
    let (world, erc1155) = deploy_testcase1();

    impersonate(USER1());

    let balance_before = erc1155.balance_of(USER2(), 1);
    erc1155.safe_transfer_from(USER1(), USER2(), 1, 1, array![]);
    let balance_after = erc1155.balance_of(USER2(), 1);

    assert(balance_after == balance_before + 1, 'invalid balance after');
}

#[test]
#[available_gas(50000000)]
fn test_safe_transfer_preserve_existing_balances() {
    // preserves existing balances which are not transferred by multiTokenHolder
    let (world, erc1155) = deploy_testcase1();

    // impersonate user1
    impersonate(USER1());

    let balance_before_2 = erc1155.balance_of(USER2(), 2);
    let balance_before_3 = erc1155.balance_of(USER2(), 3);
    erc1155.safe_transfer_from(USER1(), USER2(), 1, 1, array![]);
    let balance_after_2 = erc1155.balance_of(USER2(), 2);
    let balance_after_3 = erc1155.balance_of(USER2(), 3);

    assert(balance_after_2 == balance_before_2, 'should be equal');
    assert(balance_after_3 == balance_before_3, 'should be equal');
}

#[test]
#[available_gas(30000000)]
#[should_panic()]
fn test_safe_transfer_from_unapproved_operator() {
    // when called by an operator on behalf of the multiTokenHolder
    // when operator is not approved by multiTokenHolder

    let (world, erc1155) = deploy_testcase1();

    impersonate(USER2());

    erc1155.safe_transfer_from(USER1(), USER2(), 1, 1, array![]); // should panic
}

#[test]
#[available_gas(50000000)]
fn test_safe_transfer_from_approved_operator() {
    // when called by an operator on behalf of the multiTokenHolder
    // when operator is approved by multiTokenHolder
    let (world, erc1155) = deploy_testcase1();

    impersonate(PROXY());

    let balance_before = erc1155.balance_of(USER1(), 1);
    erc1155.safe_transfer_from(USER1(), USER2(), 1, 2, array![]);
    let balance_after = erc1155.balance_of(USER1(), 1);

    assert(balance_after == balance_before - 2, 'invalid balance');
}

#[test]
#[available_gas(50000000)]
fn test_safe_transfer_from_approved_operator_preserve_operator_balance() {
    // when called by an operator on behalf of the multiTokenHolder
    // preserves operator's balances not involved in the transfer
    let (world, erc1155) = deploy_testcase1();

    impersonate(PROXY());

    let balance_before_1 = erc1155.balance_of(PROXY(), 1);
    let balance_before_2 = erc1155.balance_of(PROXY(), 2);
    let balance_before_3 = erc1155.balance_of(PROXY(), 3);
    erc1155.safe_transfer_from(USER1(), USER2(), 1, 2, array![]);
    let balance_after_1 = erc1155.balance_of(PROXY(), 1);
    let balance_after_2 = erc1155.balance_of(PROXY(), 2);
    let balance_after_3 = erc1155.balance_of(PROXY(), 3);

    assert(balance_before_1 == balance_after_1, 'should be equal');
    assert(balance_before_2 == balance_after_2, 'should be equal');
    assert(balance_before_3 == balance_after_3, 'should be equal');
}


#[test]
#[available_gas(50000000)]
#[should_panic]
fn test_safe_transfer_from_zero_address() {
    let (world, erc1155) = deploy_testcase1();

    impersonate(USER1());

    erc1155.safe_transfer_from(ZERO(), USER1(), 1, 1, array![]);
}

//
// safe_batch_transfer_from
//

#[test]
#[available_gas(50000000)]
#[should_panic]
fn test_safe_batch_transfer_from_more_than_balance() {
    // reverts when transferring amount more than any of balances
    let (world, erc1155) = deploy_testcase1();

    impersonate(USER1());

    erc1155
        .safe_batch_transfer_from(USER1(), USER2(), array![1, 2, 3], array![1, 999, 1], array![]);
}


#[test]
#[available_gas(50000000)]
#[should_panic]
fn test_safe_batch_transfer_from_mismatching_array_len() {
    // reverts when ids array length doesn't match amounts array length
    let (world, erc1155) = deploy_testcase1();

    impersonate(USER1());

    erc1155.safe_batch_transfer_from(USER1(), USER2(), array![1, 2, 3], array![1, 1], array![]);
}


#[test]
#[available_gas(50000000)]
#[should_panic]
fn test_safe_batch_transfer_from_to_zero_address() {
    // reverts when transferring to zero address
    let (world, erc1155) = deploy_testcase1();

    impersonate(USER1());

    erc1155.safe_batch_transfer_from(USER1(), ZERO(), array![1, 2], array![1, 1], array![]);
}


#[test]
#[available_gas(60000000)]
fn test_safe_batch_transfer_from_debits_sender() {
    // debits transferred balances from sender
    let (world, erc1155) = deploy_testcase1();

    impersonate(USER1());

    let balance_before_1 = erc1155.balance_of(USER1(), 1);
    let balance_before_2 = erc1155.balance_of(USER1(), 2);
    let balance_before_3 = erc1155.balance_of(USER1(), 3);
    erc1155
        .safe_batch_transfer_from(USER1(), USER2(), array![1, 2, 3], array![1, 10, 20], array![]);
    let balance_after_1 = erc1155.balance_of(USER1(), 1);
    let balance_after_2 = erc1155.balance_of(USER1(), 2);
    let balance_after_3 = erc1155.balance_of(USER1(), 3);

    assert(balance_before_1 - 1 == balance_after_1, 'invalid balance');
    assert(balance_before_2 - 10 == balance_after_2, 'invalid balance');
    assert(balance_before_3 - 20 == balance_after_3, 'invalid balance');
}


#[test]
#[available_gas(60000000)]
fn test_safe_batch_transfer_from_credits_recipient() {
    // credits transferred balances to receiver
    let (world, erc1155) = deploy_testcase1();

    impersonate(USER1());

    let balance_before_1 = erc1155.balance_of(USER2(), 1);
    let balance_before_2 = erc1155.balance_of(USER2(), 2);
    let balance_before_3 = erc1155.balance_of(USER2(), 3);
    erc1155
        .safe_batch_transfer_from(USER1(), USER2(), array![1, 2, 3], array![1, 10, 20], array![]);
    let balance_after_1 = erc1155.balance_of(USER2(), 1);
    let balance_after_2 = erc1155.balance_of(USER2(), 2);
    let balance_after_3 = erc1155.balance_of(USER2(), 3);

    assert(balance_before_1 + 1 == balance_after_1, 'invalid balance');
    assert(balance_before_2 + 10 == balance_after_2, 'invalid balance');
    assert(balance_before_1 + 20 == balance_after_3, 'invalid balance');
}


#[test]
#[available_gas(50000000)]
#[should_panic]
fn test_safe_batch_transfer_from_unapproved_operator() {
    // when called by an operator on behalf of the multiTokenHolder
    // when operator is not approved by multiTokenHolder

    let (world, erc1155) = deploy_testcase1();

    impersonate(USER2());

    erc1155.safe_batch_transfer_from(USER1(), USER2(), array![1, 2], array![1, 10], array![]);
}

#[test]
#[available_gas(60000000)]
fn test_safe_batch_transfer_from_approved_operator_preserve_operator_balance() {
    // when called by an operator on behalf of the multiTokenHolder
    // preserves operator's balances not involved in the transfer

    let (world, erc1155) = deploy_testcase1();

    impersonate(PROXY());

    let balance_before_1 = erc1155.balance_of(PROXY(), 1);
    let balance_before_2 = erc1155.balance_of(PROXY(), 2);
    let balance_before_3 = erc1155.balance_of(PROXY(), 3);

    erc1155
        .safe_batch_transfer_from(USER1(), USER2(), array![1, 2, 3], array![1, 10, 20], array![]);

    let balance_after_1 = erc1155.balance_of(PROXY(), 1);
    let balance_after_2 = erc1155.balance_of(PROXY(), 2);
    let balance_after_3 = erc1155.balance_of(PROXY(), 3);

    assert(balance_before_1 == balance_after_1, 'should be equal');
    assert(balance_before_2 == balance_after_2, 'should be equal');
    assert(balance_before_3 == balance_after_3, 'should be equal');
}

#[test]
#[available_gas(50000000)]
#[should_panic]
fn test_safe_batch_transfer_from_zero_address() {
    let (world, erc1155) = deploy_testcase1();

    impersonate(USER1());

    erc1155.safe_batch_transfer_from(ZERO(), USER1(), array![1, 2], array![1, 1], array![]);
}


#[test]
#[available_gas(50000000)]
fn test_safe_batch_transfer_emit_transfer_batch_event() {
    let (world, erc1155) = deploy_default();

    // user1  token_id 1  x 10
    erc1155.mint(USER1(), 1, 10, array![]);
    // user1  token_id 2 x 20
    erc1155.mint(USER1(), 2, 20, array![]);

    impersonate(USER1());

    erc1155.safe_batch_transfer_from(USER1(), USER2(), array![1, 2], array![1, 10], array![]);

    let _: Event = starknet::testing::pop_log(erc1155.contract_address)
        .unwrap(); // unpop   erc1155.mint(USER1(), 1, 10, array![]);
    let _: Event = starknet::testing::pop_log(erc1155.contract_address)
        .unwrap(); // unpop   erc1155.mint(USER1(), 2, 20, array![]);

    // TransferBatch
    assert(
        @starknet::testing::pop_log(erc1155.contract_address)
            .unwrap() == @Event::TransferBatch(
                TransferBatch {
                    operator: USER1(),
                    from: USER1(),
                    to: USER2(),
                    ids: array![1, 2],
                    values: array![1, 10]
                }
            ),
        'invalid TransferBatch event'
    );
}


//
// burn
//

#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_burn_non_existing_token_id() {
    //reverts when burning a non-existent token id
    let (world, erc1155) = deploy_default();

    impersonate(USER1());
    erc1155.burn(USER1(), 69, 1); // should panic
}


#[test]
#[available_gas(90000000)]
fn test_burn_emit_transfer_single_event() {
    // burn should emit event
    let (world, erc1155) = deploy_default();

    erc1155.mint(USER1(), 69, 5, array![]);
    assert(erc1155.balance_of(USER1(), 69) == 5, 'invalid balance');

    impersonate(USER1());

    erc1155.burn(USER1(), 69, 1);
    assert(erc1155.balance_of(USER1(), 69) == 4, 'invalid balance');

    let _: Event = starknet::testing::pop_log(erc1155.contract_address)
        .unwrap(); // unpop  erc1155.mint(USER1(), 69,5,array![])

    // TransferSingle 
    assert(
        @starknet::testing::pop_log(erc1155.contract_address)
            .unwrap() == @Event::TransferSingle(
                TransferSingle { operator: USER1(), from: USER1(), to: ZERO(), id: 69, value: 1 }
            ),
        'invalid TransferSingle event'
    );
}


#[test]
#[available_gas(90000000)]
#[should_panic]
fn test_burn_more_than_owned() {
    // reverts when burning more tokens than owned
    let (world, erc1155) = deploy_default();
    erc1155.mint(USER1(), 69, 10, array![]);

    impersonate(USER1());

    erc1155.burn(USER1(), 69, 1);
    erc1155.burn(USER1(), 69, 10); // should panic
}
// TODO : to be continued

// TODO : add test if we support IERC1155Receiver



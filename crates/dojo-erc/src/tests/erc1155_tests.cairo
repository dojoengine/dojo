use dojo_erc::tests::utils;
use dojo_erc::tests::constants::{
    ZERO, OWNER, SPENDER, RECIPIENT, OPERATOR, OTHER, NAME, SYMBOL, URI, TOKEN_ID, TOKEN_AMOUNT,
    TOKEN_ID_2, TOKEN_AMOUNT_2
};

use dojo_erc::token::erc1155::ERC1155::ERC1155Impl;
use dojo_erc::token::erc1155::ERC1155::ERC1155CamelOnlyImpl;
use dojo_erc::token::erc1155::ERC1155::ERC1155MetadataImpl;
use dojo_erc::token::erc1155::ERC1155::InternalImpl;
use dojo_erc::token::erc1155::ERC1155::WorldInteractionsImpl;
use dojo_erc::token::erc1155::ERC1155::{TransferSingle, TransferBatch, ApprovalForAll};
use dojo_erc::token::erc1155::ERC1155;
use starknet::ContractAddress;
use starknet::contract_address_const;
use starknet::testing;
use zeroable::Zeroable;
use dojo::test_utils::spawn_test_world;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo_erc::token::erc1155::models::{
    ERC1155Meta, erc_1155_meta, ERC1155OperatorApproval, erc_1155_operator_approval, ERC1155Balance,
    erc_1155_balance
};
use dojo_erc::token::erc1155::ERC1155::_worldContractMemberStateTrait;
use debug::PrintTrait;

//
// Setup
//

fn STATE() -> (IWorldDispatcher, ERC1155::ContractState) {
    let world = spawn_test_world(
        array![
            erc_1155_meta::TEST_CLASS_HASH,
            erc_1155_operator_approval::TEST_CLASS_HASH,
            erc_1155_balance::TEST_CLASS_HASH,
        ]
    );
    let mut state = ERC1155::contract_state_for_testing();
    state._world.write(world.contract_address);

    InternalImpl::_mint(ref state, OWNER(), TOKEN_ID, TOKEN_AMOUNT);
    utils::drop_event(ZERO());

    InternalImpl::_mint(ref state, OWNER(), TOKEN_ID_2, TOKEN_AMOUNT_2);
    utils::drop_event(ZERO());

    (world, state)
}

fn setup() -> ERC1155::ContractState {
    let (world, mut state) = STATE();
    ERC1155::constructor(ref state, world.contract_address, NAME, SYMBOL, URI);
    utils::drop_event(ZERO());
    state
}

//
// initializer & constructor
//

#[test]
#[available_gas(20000000)]
fn test_constructor() {
    let (world, mut state) = STATE();
    ERC1155::constructor(ref state, world.contract_address, NAME, SYMBOL, URI);

    assert(ERC1155MetadataImpl::name(@state) == NAME, 'Name should be NAME');
    assert(ERC1155MetadataImpl::symbol(@state) == SYMBOL, 'Symbol should be SYMBOL');
    assert(ERC1155MetadataImpl::uri(@state, 0) == URI, 'Uri should be URI');
// assert(
//     SRC5Impl::supports_interface(@state, erc1155::interface::IERC1155_ID), 'Missing interface ID'
// );
// assert(
//     SRC5Impl::supports_interface(@state, erc1155::interface::IERC1155_METADATA_ID),
//     'missing interface ID'
// );
// assert(
//     SRC5Impl::supports_interface(@state, introspection::interface::ISRC5_ID),
//     'missing interface ID'
// );
}

#[test]
#[available_gas(20000000)]
fn test_initializer() {
    let (world, mut state) = STATE();
    InternalImpl::initializer(ref state, NAME, SYMBOL, URI);

    assert(ERC1155MetadataImpl::name(@state) == NAME, 'Name should be NAME');
    assert(ERC1155MetadataImpl::symbol(@state) == SYMBOL, 'Symbol should be SYMBOL');

    assert(ERC1155Impl::balance_of(@state, OWNER(), 0) == 0, 'Balance should be zero');
    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID) == TOKEN_AMOUNT, 'should be TOKEN_AMOUNT'
    );
    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID_2) == TOKEN_AMOUNT_2,
        'should be TOKEN_AMOUNT_2'
    );
}


//
// Getters
//

#[test]
#[available_gas(20000000)]
fn test_balance_of() {
    let mut state = setup();

    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID) == TOKEN_AMOUNT, 'Should return balance'
    );
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: invalid account',))]
fn test_balance_of_zero() {
    let state = setup();
    ERC1155Impl::balance_of(@state, ZERO(), TOKEN_ID);
}


#[test]
#[available_gas(20000000)]
fn test_balance_of_batch() {
    let mut state = setup();

    InternalImpl::_mint(ref state, OTHER(), TOKEN_ID_2, TOKEN_AMOUNT_2);

    let balances = ERC1155Impl::balance_of_batch(
        @state, array![OWNER(), OTHER()], array![TOKEN_ID, TOKEN_ID_2]
    );

    assert(*balances.at(0) == TOKEN_AMOUNT, 'Should return TOKEN_AMOUNT');
    assert(*balances.at(1) == TOKEN_AMOUNT_2, 'Should return TOKEN_AMOUNT_2');
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: invalid account',))]
fn test_balance_of_batch_zero() {
    let state = setup();
    ERC1155Impl::balance_of_batch(@state, array![OTHER(), ZERO()], array![TOKEN_ID_2, TOKEN_ID]);
}


//
// set_approval_for_all & _set_approval_for_all
//

#[test]
#[available_gas(20000000)]
fn test_set_approval_for_all() {
    let (world, mut state) = STATE();
    testing::set_caller_address(OWNER());

    assert(!ERC1155Impl::is_approved_for_all(@state, OWNER(), OPERATOR()), 'Invalid default value');

    ERC1155Impl::set_approval_for_all(ref state, OPERATOR(), true);
    assert_event_approval_for_all(OWNER(), OPERATOR(), true);

    assert(
        ERC1155Impl::is_approved_for_all(@state, OWNER(), OPERATOR()),
        'Operator not approved correctly'
    );

    ERC1155Impl::set_approval_for_all(ref state, OPERATOR(), false);
    assert_event_approval_for_all(OWNER(), OPERATOR(), false);

    assert(
        !ERC1155Impl::is_approved_for_all(@state, OWNER(), OPERATOR()),
        'Approval not revoked correctly'
    );
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: self approval',))]
fn test_set_approval_for_all_owner_equal_operator_true() {
    let (world, mut state) = STATE();
    testing::set_caller_address(OWNER());
    ERC1155Impl::set_approval_for_all(ref state, OWNER(), true);
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: self approval',))]
fn test_set_approval_for_all_owner_equal_operator_false() {
    let (world, mut state) = STATE();
    testing::set_caller_address(OWNER());
    ERC1155Impl::set_approval_for_all(ref state, OWNER(), false);
}

#[test]
#[available_gas(20000000)]
fn test__set_approval_for_all() {
    let (world, mut state) = STATE();
    assert(!ERC1155Impl::is_approved_for_all(@state, OWNER(), OPERATOR()), 'Invalid default value');

    InternalImpl::_set_approval_for_all(ref state, OWNER(), OPERATOR(), true);
    assert_event_approval_for_all(OWNER(), OPERATOR(), true);

    assert(
        ERC1155Impl::is_approved_for_all(@state, OWNER(), OPERATOR()),
        'Operator not approved correctly'
    );

    InternalImpl::_set_approval_for_all(ref state, OWNER(), OPERATOR(), false);
    assert_event_approval_for_all(OWNER(), OPERATOR(), false);

    assert(
        !ERC1155Impl::is_approved_for_all(@state, OWNER(), OPERATOR()),
        'Operator not approved correctly'
    );
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: self approval',))]
fn test__set_approval_for_all_owner_equal_operator_true() {
    let (world, mut state) = STATE();
    InternalImpl::_set_approval_for_all(ref state, OWNER(), OWNER(), true);
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: self approval',))]
fn test__set_approval_for_all_owner_equal_operator_false() {
    let (world, mut state) = STATE();
    InternalImpl::_set_approval_for_all(ref state, OWNER(), OWNER(), false);
}


//
// safe_transfer_from & safeTransferFrom
//

#[test]
#[available_gas(50000000)]
fn test_safe_transfer_from_owner() {
    let mut state = setup();
    let id = TOKEN_ID;
    let amount = TOKEN_AMOUNT;
    let owner = OWNER();
    let recipient = RECIPIENT();

    assert_state_before_transfer(@state, owner, recipient, id);

    testing::set_caller_address(owner);
    ERC1155Impl::safe_transfer_from(ref state, owner, recipient, id, amount, array![]);
    assert_event_transfer_single(owner, recipient, id, amount);

    assert_state_after_transfer(@state, owner, recipient, id);
}

#[test]
#[available_gas(50000000)]
fn test_transferFrom_owner() {
    let mut state = setup();
    let id = TOKEN_ID;
    let amount = TOKEN_AMOUNT;
    let owner = OWNER();
    let recipient = RECIPIENT();

    assert_state_before_transfer(@state, owner, recipient, id);

    testing::set_caller_address(owner);
    ERC1155CamelOnlyImpl::safeTransferFrom(ref state, owner, recipient, id, amount, array![]);
    assert_event_transfer_single(owner, recipient, id, amount);

    assert_state_after_transfer(@state, owner, recipient, id);
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: wrong sender',))]
fn test_safe_transfer_from_zero() {
    let (world, mut state) = STATE();
    ERC1155Impl::safe_transfer_from(
        ref state, ZERO(), RECIPIENT(), TOKEN_ID, TOKEN_AMOUNT, array![]
    );
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: wrong sender',))]
fn test_safeTransferFrom_zero() {
    let (world, mut state) = STATE();
    ERC1155CamelOnlyImpl::safeTransferFrom(
        ref state, ZERO(), RECIPIENT(), TOKEN_ID, TOKEN_AMOUNT, array![]
    );
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: invalid receiver',))]
fn test_safe_transfer_from_to_zero() {
    let mut state = setup();
    testing::set_caller_address(OWNER());
    ERC1155Impl::safe_transfer_from(ref state, OWNER(), ZERO(), TOKEN_ID, TOKEN_AMOUNT, array![]);
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: invalid receiver',))]
fn test_safeTransferFrom_to_zero() {
    let mut state = setup();
    testing::set_caller_address(OWNER());
    ERC1155CamelOnlyImpl::safeTransferFrom(
        ref state, OWNER(), ZERO(), TOKEN_ID, TOKEN_AMOUNT, array![]
    );
}

#[test]
#[available_gas(50000000)]
fn test_safe_transfer_from_to_owner() {
    let mut state = setup();

    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID) == TOKEN_AMOUNT,
        'Balance of owner before'
    );

    testing::set_caller_address(OWNER());
    ERC1155Impl::safe_transfer_from(ref state, OWNER(), OWNER(), TOKEN_ID, TOKEN_AMOUNT, array![]);
    assert_event_transfer_single(OWNER(), OWNER(), TOKEN_ID, TOKEN_AMOUNT);

    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID) == TOKEN_AMOUNT, 'Balance of owner after'
    );
}

#[test]
#[available_gas(50000000)]
fn test_safeTransferFrom_to_owner() {
    let mut state = setup();

    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID) == TOKEN_AMOUNT,
        'Balance of owner before'
    );

    testing::set_caller_address(OWNER());
    ERC1155CamelOnlyImpl::safeTransferFrom(
        ref state, OWNER(), OWNER(), TOKEN_ID, TOKEN_AMOUNT, array![]
    );
    assert_event_transfer_single(OWNER(), OWNER(), TOKEN_ID, TOKEN_AMOUNT);

    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID) == TOKEN_AMOUNT, 'Balance of owner after'
    );
}

#[test]
#[available_gas(50000000)]
fn test_transfer_from_approved_for_all() {
    let mut state = setup();
    let id = TOKEN_ID;
    let amount = TOKEN_AMOUNT;
    let owner = OWNER();
    let recipient = RECIPIENT();

    assert_state_before_transfer(@state, owner, recipient, id);

    testing::set_caller_address(owner);
    ERC1155Impl::set_approval_for_all(ref state, OPERATOR(), true);
    utils::drop_event(ZERO());

    testing::set_caller_address(OPERATOR());
    ERC1155Impl::safe_transfer_from(ref state, owner, recipient, id, amount, array![]);
    assert_event_transfer_single(owner, recipient, id, amount);

    assert_state_after_transfer(@state, owner, recipient, id);
}

#[test]
#[available_gas(50000000)]
fn test_safeTransferFrom_approved_for_all() {
    let mut state = setup();
    let id = TOKEN_ID;
    let amount = TOKEN_AMOUNT;
    let owner = OWNER();
    let recipient = RECIPIENT();

    assert_state_before_transfer(@state, owner, recipient, id);

    testing::set_caller_address(owner);
    ERC1155CamelOnlyImpl::setApprovalForAll(ref state, OPERATOR(), true);
    utils::drop_event(ZERO());

    testing::set_caller_address(OPERATOR());
    ERC1155CamelOnlyImpl::safeTransferFrom(ref state, owner, recipient, id, amount, array![]);
    assert_event_transfer_single(owner, recipient, id, amount);

    assert_state_after_transfer(@state, owner, recipient, id);
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: unauthorized caller',))]
fn test_safe_transfer_from_unauthorized() {
    let mut state = setup();
    testing::set_caller_address(OTHER());
    ERC1155Impl::safe_transfer_from(
        ref state, OWNER(), RECIPIENT(), TOKEN_ID, TOKEN_AMOUNT, array![]
    );
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: unauthorized caller',))]
fn test_safeTransferFrom_unauthorized() {
    let mut state = setup();
    testing::set_caller_address(OTHER());
    ERC1155CamelOnlyImpl::safeTransferFrom(
        ref state, OWNER(), RECIPIENT(), TOKEN_ID, TOKEN_AMOUNT, array![]
    );
}


//
// safe_batch_transfer_from & safeBatchTransferFrom
//

#[test]
#[available_gas(50000000)]
fn test_safe_batch_transfer_from_owner() {
    let mut state = setup();
    let owner = OWNER();
    let recipient = RECIPIENT();

    let ids = array![TOKEN_ID, TOKEN_ID_2];
    let amounts = array![TOKEN_AMOUNT, TOKEN_AMOUNT_2];

    assert_state_before_batch_transfer(@state, owner, recipient);

    testing::set_caller_address(owner);
    ERC1155Impl::safe_batch_transfer_from(
        ref state, owner, recipient, ids.clone(), amounts.clone(), array![]
    );
    assert_event_transfer_batch(owner, recipient, ids, amounts);

    assert_state_after_batch_transfer(@state, owner, recipient);
}

#[test]
#[available_gas(50000000)]
fn test_safeBatchTransferFrom_owner() {
    let mut state = setup();
    let owner = OWNER();
    let recipient = RECIPIENT();

    let ids = array![TOKEN_ID, TOKEN_ID_2];
    let amounts = array![TOKEN_AMOUNT, TOKEN_AMOUNT_2];

    assert_state_before_batch_transfer(@state, owner, recipient);

    testing::set_caller_address(owner);
    ERC1155CamelOnlyImpl::safeBatchTransferFrom(
        ref state, owner, recipient, ids.clone(), amounts.clone(), array![]
    );
    assert_event_transfer_batch(owner, recipient, ids, amounts);

    assert_state_after_batch_transfer(@state, owner, recipient);
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: wrong sender',))]
fn test_safe_batch_transfer_from_zero() {
    let (world, mut state) = STATE();

    let ids = array![TOKEN_ID, TOKEN_ID_2];
    let amounts = array![TOKEN_AMOUNT, TOKEN_AMOUNT_2];

    ERC1155Impl::safe_batch_transfer_from(ref state, ZERO(), RECIPIENT(), ids, amounts, array![]);
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: wrong sender',))]
fn test_safeBatchTransferFrom_zero() {
    let (world, mut state) = STATE();

    let ids = array![TOKEN_ID, TOKEN_ID_2];
    let amounts = array![TOKEN_AMOUNT, TOKEN_AMOUNT_2];

    ERC1155CamelOnlyImpl::safeBatchTransferFrom(
        ref state, ZERO(), RECIPIENT(), ids, amounts, array![]
    );
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: invalid receiver',))]
fn test_safe_batch_transfer_from_to_zero() {
    let (world, mut state) = STATE();

    let ids = array![TOKEN_ID, TOKEN_ID_2];
    let amounts = array![TOKEN_AMOUNT, TOKEN_AMOUNT_2];

    ERC1155Impl::safe_batch_transfer_from(ref state, OWNER(), ZERO(), ids, amounts, array![]);
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: invalid receiver',))]
fn test_safeBatchTransferFrom_to_zero() {
    let (world, mut state) = STATE();

    let ids = array![TOKEN_ID, TOKEN_ID_2];
    let amounts = array![TOKEN_AMOUNT, TOKEN_AMOUNT_2];

    ERC1155Impl::safe_batch_transfer_from(ref state, OWNER(), ZERO(), ids, amounts, array![]);
}

#[test]
#[available_gas(50000000)]
fn test_safe_batch_transfer_from_to_owner() {
    let mut state = setup();

    let ids = array![TOKEN_ID, TOKEN_ID_2];
    let amounts = array![TOKEN_AMOUNT, TOKEN_AMOUNT_2];

    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID) == TOKEN_AMOUNT,
        'Balance of owner before1'
    );
    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID_2) == TOKEN_AMOUNT_2,
        'Balance of owner before1'
    );

    testing::set_caller_address(OWNER());
    ERC1155Impl::safe_batch_transfer_from(
        ref state, OWNER(), OWNER(), ids.clone(), amounts.clone(), array![]
    );
    assert_event_transfer_batch(OWNER(), OWNER(), ids, amounts);

    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID) == TOKEN_AMOUNT,
        'Balance of owner after1'
    );
    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID_2) == TOKEN_AMOUNT_2,
        'Balance of owner after2'
    );
}

#[test]
#[available_gas(50000000)]
fn test_safeBatchTransferFrom_to_owner() {
    let mut state = setup();

    let ids = array![TOKEN_ID, TOKEN_ID_2];
    let amounts = array![TOKEN_AMOUNT, TOKEN_AMOUNT_2];

    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID) == TOKEN_AMOUNT,
        'Balance of owner before1'
    );
    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID_2) == TOKEN_AMOUNT_2,
        'Balance of owner before1'
    );

    testing::set_caller_address(OWNER());
    ERC1155CamelOnlyImpl::safeBatchTransferFrom(
        ref state, OWNER(), OWNER(), ids.clone(), amounts.clone(), array![]
    );
    assert_event_transfer_batch(OWNER(), OWNER(), ids, amounts);

    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID) == TOKEN_AMOUNT,
        'Balance of owner after1'
    );
    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID_2) == TOKEN_AMOUNT_2,
        'Balance of owner after2'
    );
}

#[test]
#[available_gas(50000000)]
fn test_batch_transfer_from_approved_for_all() {
    let mut state = setup();
    let owner = OWNER();
    let recipient = RECIPIENT();

    let ids = array![TOKEN_ID, TOKEN_ID_2];
    let amounts = array![TOKEN_AMOUNT, TOKEN_AMOUNT_2];

    assert_state_before_batch_transfer(@state, owner, recipient);

    testing::set_caller_address(owner);
    ERC1155Impl::set_approval_for_all(ref state, OPERATOR(), true);
    utils::drop_event(ZERO());

    testing::set_caller_address(OPERATOR());
    ERC1155Impl::safe_batch_transfer_from(
        ref state, owner, recipient, ids.clone(), amounts.clone(), array![]
    );
    assert_event_transfer_batch(owner, recipient, ids, amounts);

    assert_state_after_batch_transfer(@state, owner, recipient);
}

#[test]
#[available_gas(50000000)]
fn test_safeBatchTransferFrom_approved_for_all() {
    let mut state = setup();
    let owner = OWNER();
    let recipient = RECIPIENT();

    let ids = array![TOKEN_ID, TOKEN_ID_2];
    let amounts = array![TOKEN_AMOUNT, TOKEN_AMOUNT_2];

    assert_state_before_batch_transfer(@state, owner, recipient);

    testing::set_caller_address(owner);
    ERC1155CamelOnlyImpl::setApprovalForAll(ref state, OPERATOR(), true);
    utils::drop_event(ZERO());

    testing::set_caller_address(OPERATOR());
    ERC1155CamelOnlyImpl::safeBatchTransferFrom(
        ref state, owner, recipient, ids.clone(), amounts.clone(), array![]
    );
    assert_event_transfer_batch(owner, recipient, ids, amounts);

    assert_state_after_batch_transfer(@state, owner, recipient);
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: unauthorized caller',))]
fn test_safe_batch_transfer_from_unauthorized() {
    let mut state = setup();
    let ids = array![TOKEN_ID, TOKEN_ID_2];
    let amounts = array![TOKEN_AMOUNT, TOKEN_AMOUNT_2];

    testing::set_caller_address(OTHER());
    ERC1155Impl::safe_batch_transfer_from(ref state, OWNER(), RECIPIENT(), ids, amounts, array![]);
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: unauthorized caller',))]
fn test_safeBatchTransferFrom_unauthorized() {
    let mut state = setup();
    let ids = array![TOKEN_ID, TOKEN_ID_2];
    let amounts = array![TOKEN_AMOUNT, TOKEN_AMOUNT_2];

    testing::set_caller_address(OTHER());
    ERC1155CamelOnlyImpl::safeBatchTransferFrom(
        ref state, OWNER(), RECIPIENT(), ids, amounts, array![]
    );
}

//
// _mint
//

#[test]
#[available_gas(20000000)]
fn test__mint() {
    let (world, mut state) = STATE();
    let recipient = RECIPIENT();

    assert(
        ERC1155Impl::balance_of(@state, recipient, TOKEN_ID_2) == 0, 'Balance of recipient before'
    );

    InternalImpl::_mint(ref state, recipient, TOKEN_ID_2, TOKEN_AMOUNT_2);
    assert_event_transfer_single(ZERO(), recipient, TOKEN_ID_2, TOKEN_AMOUNT_2);

    assert(
        ERC1155Impl::balance_of(@state, recipient, TOKEN_ID_2) == TOKEN_AMOUNT_2,
        'Balance of recipient after'
    );
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: invalid receiver',))]
fn test__mint_to_zero() {
    let (world, mut state) = STATE();
    InternalImpl::_mint(ref state, ZERO(), TOKEN_ID, TOKEN_AMOUNT);
}


//
// _burn
//

#[test]
#[available_gas(20000000)]
fn test__burn() {
    let mut state = setup();

    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID) == TOKEN_AMOUNT,
        'Balance of owner before'
    );

    testing::set_caller_address(OWNER());
    InternalImpl::_burn(ref state, TOKEN_ID, 2);
    assert_event_transfer_single(OWNER(), ZERO(), TOKEN_ID, 2);

    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID) == TOKEN_AMOUNT - 2,
        'Balance of owner after'
    );
}

#[test]
#[available_gas(20000000)]
#[should_panic(expected: ('ERC1155: insufficient balance',))]
fn test__burn_more_than_balance() {
    let mut state = setup();

    assert(
        ERC1155Impl::balance_of(@state, OWNER(), TOKEN_ID) == TOKEN_AMOUNT,
        'Balance of owner before'
    );
    InternalImpl::_burn(ref state, TOKEN_ID, TOKEN_AMOUNT + 1);
}


//
// Helpers 
//

fn assert_state_before_transfer(
    state: @ERC1155::ContractState, owner: ContractAddress, recipient: ContractAddress, id: u256,
) {
    assert(ERC1155Impl::balance_of(state, owner, id) == TOKEN_AMOUNT, 'Balance of owner before');
    assert(ERC1155Impl::balance_of(state, recipient, id) == 0, 'Balance of recipient before');
}

fn assert_state_after_transfer(
    state: @ERC1155::ContractState, owner: ContractAddress, recipient: ContractAddress, id: u256
) {
    assert(ERC1155Impl::balance_of(state, owner, id) == 0, 'Balance of owner after');
    assert(
        ERC1155Impl::balance_of(state, recipient, id) == TOKEN_AMOUNT, 'Balance of recipient after'
    );
}

fn assert_state_before_batch_transfer(
    state: @ERC1155::ContractState, owner: ContractAddress, recipient: ContractAddress
) {
    assert(
        ERC1155Impl::balance_of(state, owner, TOKEN_ID) == TOKEN_AMOUNT, 'Balance of owner before1'
    );
    assert(
        ERC1155Impl::balance_of(state, owner, TOKEN_ID_2) == TOKEN_AMOUNT_2,
        'Balance of owner before2'
    );
    assert(
        ERC1155Impl::balance_of(state, recipient, TOKEN_ID) == 0, 'Balance of recipient before1'
    );
    assert(
        ERC1155Impl::balance_of(state, recipient, TOKEN_ID_2) == 0, 'Balance of recipient before2'
    );
}

fn assert_state_after_batch_transfer(
    state: @ERC1155::ContractState, owner: ContractAddress, recipient: ContractAddress
) {
    assert(ERC1155Impl::balance_of(state, owner, TOKEN_ID) == 0, 'Balance of owner after1');
    assert(ERC1155Impl::balance_of(state, owner, TOKEN_ID_2) == 0, 'Balance of owner after2');
    assert(
        ERC1155Impl::balance_of(state, recipient, TOKEN_ID) == TOKEN_AMOUNT,
        'Balance of recipient after1'
    );
    assert(
        ERC1155Impl::balance_of(state, recipient, TOKEN_ID_2) == TOKEN_AMOUNT_2,
        'Balance of recipient after2'
    );
}


//
// events
//

fn assert_event_approval_for_all(
    owner: ContractAddress, operator: ContractAddress, approved: bool
) {
    let event = utils::pop_log::<ApprovalForAll>(ZERO()).unwrap();
    assert(event.owner == owner, 'Invalid `owner`');
    assert(event.operator == operator, 'Invalid `operator`');
    assert(event.approved == approved, 'Invalid `approved`');
    utils::assert_no_events_left(ZERO());
}

fn assert_event_transfer_single(
    from: ContractAddress, to: ContractAddress, id: u256, amount: u256
) {
    let event = utils::pop_log::<TransferSingle>(ZERO()).unwrap();
    assert(event.from == from, 'Invalid `from`');
    assert(event.to == to, 'Invalid `to`');
    assert(event.id == id, 'Invalid `id`');
    assert(event.value == amount, 'Invalid `amount`');
    utils::assert_no_events_left(ZERO());
}

fn assert_event_transfer_batch(
    from: ContractAddress, to: ContractAddress, ids: Array<u256>, amounts: Array<u256>
) {
    let event = utils::pop_log::<TransferBatch>(ZERO()).unwrap();
    assert(event.from == from, 'Invalid `from`');
    assert(event.to == to, 'Invalid `to`');
    assert(event.ids.len() == event.values.len(), 'Invalid array length');

    let mut i = 0;

    loop {
        if i == event.ids.len() {
            break;
        }

        assert(event.ids.at(i) == ids.at(i), 'Invalid `id`');
        assert(event.values.at(i) == amounts.at(i), 'Invalid `id`');

        i += 1;
    };

    utils::assert_no_events_left(ZERO());
}


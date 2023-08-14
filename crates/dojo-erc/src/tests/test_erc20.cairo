use array::ArrayTrait;
use debug::PrintTrait;
use integer::BoundedInt;
use option::OptionTrait;
use result::ResultTrait;
use starknet::{ContractAddress, contract_address_const, get_caller_address, get_contract_address};
use starknet::class_hash::ClassHash;
use starknet::class_hash::Felt252TryIntoClassHash;
use starknet::syscalls::deploy_syscall;
use starknet::SyscallResultTrait;
use starknet::testing::set_contract_address;
use traits::{Into, TryInto};
use zeroable::Zeroable;
use dojo_erc::erc20::erc20::{IERC20Dispatcher, IERC20DispatcherTrait, ERC20};
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
use dojo_erc::tests::test_utils::{
    NAME, SYMBOL, DECIMALS, OWNER, SPENDER, SUPPLY, RECIPIENT, VALUE, deploy_erc20
};

#[test]
#[available_gas(200000000)]
fn test_constructor() {
    let (world, erc20_address) = deploy_erc20();
    assert(
        IERC20Dispatcher { contract_address: erc20_address }.balance_of(OWNER()) == SUPPLY,
        'Should eq inital_supply'
    );
    assert(
        IERC20Dispatcher { contract_address: erc20_address }.total_supply() == SUPPLY,
        'Should eq inital_supply'
    );
    assert(
        IERC20Dispatcher { contract_address: erc20_address }.name() == NAME, 'Name Should be NAME'
    );
    assert(
        IERC20Dispatcher { contract_address: erc20_address }.symbol() == SYMBOL,
        'Symbol Should be SYMBOL'
    );
    assert(
        IERC20Dispatcher { contract_address: erc20_address }.decimals() == DECIMALS,
        'Decimals Should be 18'
    );
}

#[test]
#[available_gas(200000000)]
fn test_allowance() {
    let (world, erc20_address) = deploy_erc20();
    set_contract_address(OWNER());
    IERC20Dispatcher { contract_address: erc20_address }.approve(SPENDER(), VALUE);
    assert(
        IERC20Dispatcher { contract_address: erc20_address }.allowance(OWNER(), SPENDER()) == VALUE,
        'Should eq VALUE'
    );
}

#[test]
#[available_gas(200000000)]
fn test_approve() {
    let (world, erc20_address) = deploy_erc20();
    set_contract_address(OWNER());
    assert(
        IERC20Dispatcher { contract_address: erc20_address }.approve(SPENDER(), VALUE),
        'Should return true'
    );
    assert(
        IERC20Dispatcher { contract_address: erc20_address }.allowance(OWNER(), SPENDER()) == VALUE,
        'Spender not approved correctly'
    );
}

#[test]
#[available_gas(200000000)]
#[should_panic(expected: ('ERC20: approve from 0', 'ENTRYPOINT_FAILED'))]
fn test_approve_from_zero() {
    let (world, erc20_address) = deploy_erc20();
    IERC20Dispatcher { contract_address: erc20_address }.approve(SPENDER(), VALUE);
}

#[test]
#[available_gas(200000000)]
#[should_panic(expected: ('ERC20: approve to 0', 'ENTRYPOINT_FAILED'))]
fn test_approve_to_zero() {
    set_contract_address(OWNER());
    let (world, erc20_address) = deploy_erc20();
    IERC20Dispatcher { contract_address: erc20_address }.approve(Zeroable::zero(), VALUE);
}

#[test]
#[available_gas(200000000)]
fn test_transfer() {
    let (world, erc20_address) = deploy_erc20();
    set_contract_address(OWNER());
    assert(
        IERC20Dispatcher { contract_address: erc20_address }.transfer(RECIPIENT(), VALUE),
        'Should return true'
    );
}

#[test]
#[available_gas(2000000000)]
#[should_panic(expected: ('ERC20: not enough balance', 'ENTRYPOINT_FAILED'))]
fn test_transfer_not_enough_balance() {
    let (world, erc20_address) = deploy_erc20();
    set_contract_address(OWNER());
    let balance_plus_one = SUPPLY + 1;
    IERC20Dispatcher {
        contract_address: erc20_address
    }.transfer(RECIPIENT(), balance_plus_one.into());
}

#[test]
#[available_gas(2000000000)]
#[should_panic(expected: ('ERC20: transfer from 0', 'ENTRYPOINT_FAILED'))]
fn test_transfer_from_zero() {
    let (world, erc20_address) = deploy_erc20();
    IERC20Dispatcher { contract_address: erc20_address }.transfer(RECIPIENT(), VALUE);
}

#[test]
#[available_gas(2000000000)]
#[should_panic(expected: ('ERC20: transfer to 0', 'ENTRYPOINT_FAILED'))]
fn test_transfer_to_zero() {
    let (world, erc20_address) = deploy_erc20();
    set_contract_address(RECIPIENT());
    IERC20Dispatcher { contract_address: erc20_address }.transfer(Zeroable::zero(), VALUE);
}

#[test]
#[available_gas(200000000)]
fn test_transfer_from() {
    let (world, erc20_address) = deploy_erc20();
    set_contract_address(OWNER());
    IERC20Dispatcher { contract_address: erc20_address }.approve(SPENDER(), VALUE);

    set_contract_address(SPENDER());
    assert(
        IERC20Dispatcher {
            contract_address: erc20_address
        }.transfer_from(OWNER(), RECIPIENT(), VALUE),
        'Should return true'
    );
    assert(
        IERC20Dispatcher { contract_address: erc20_address }.balance_of(RECIPIENT()) == VALUE,
        'Should eq amount'
    );
    assert(
        IERC20Dispatcher {
            contract_address: erc20_address
        }.balance_of(OWNER()) == (SUPPLY - VALUE).into(),
        'Should eq suppy - amount'
    );
    assert(
        IERC20Dispatcher {
            contract_address: erc20_address
        }.allowance(OWNER(), SPENDER()) == 0.into(),
        'Should eq to 0'
    );
    assert(
        IERC20Dispatcher { contract_address: erc20_address }.total_supply() == SUPPLY,
        'Total supply should not change'
    );
}

#[test]
#[available_gas(200000000)]
fn test_transfer_from_doesnt_consume_infinite_allowance() {
    let (world, erc20_address) = deploy_erc20();
    set_contract_address(OWNER());
    IERC20Dispatcher { contract_address: erc20_address }.approve(SPENDER(), BoundedInt::max());

    set_contract_address(SPENDER());
    assert(
        IERC20Dispatcher {
            contract_address: erc20_address
        }.transfer_from(OWNER(), RECIPIENT(), VALUE),
        'Should return true'
    );
    assert(
        IERC20Dispatcher {
            contract_address: erc20_address
        }.allowance(OWNER(), SPENDER()) == ERC20::UNLIMITED_ALLOWANCE.into(),
        'allowance should not change'
    );
}

#[test]
#[available_gas(200000000)]
#[should_panic(expected: ('u256_sub Overflow', 'ENTRYPOINT_FAILED'))]
fn test_transfer_from_greater_than_allowance() {
    let (world, erc20_address) = deploy_erc20();
    set_contract_address(OWNER());
    IERC20Dispatcher { contract_address: erc20_address }.approve(SPENDER(), VALUE);

    set_contract_address(SPENDER());
    let allowance_plus_one = VALUE + 1;
    IERC20Dispatcher {
        contract_address: erc20_address
    }.transfer_from(OWNER(), RECIPIENT(), allowance_plus_one);
}

#[test]
#[available_gas(200000000)]
#[should_panic(expected: ('ERC20: transfer to 0', 'ENTRYPOINT_FAILED'))]
fn test_transfer_from_to_zero_address() {
    let (world, erc20_address) = deploy_erc20();
    set_contract_address(OWNER());
    IERC20Dispatcher { contract_address: erc20_address }.approve(SPENDER(), VALUE);

    set_contract_address(SPENDER());
    IERC20Dispatcher {
        contract_address: erc20_address
    }.transfer_from(OWNER(), Zeroable::zero(), VALUE);
}

#[test]
#[available_gas(200000000)]
#[should_panic(expected: ('u256_sub Overflow', 'ENTRYPOINT_FAILED'))]
fn test_transfer_from_from_zero_address() {
    let (world, erc20_address) = deploy_erc20();
    IERC20Dispatcher {
        contract_address: erc20_address
    }.transfer_from(Zeroable::zero(), RECIPIENT(), VALUE);
}

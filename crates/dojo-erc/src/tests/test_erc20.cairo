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

use dojo_erc::erc20::erc20::ERC20;
use dojo_erc::erc20::interface::{IERC20Dispatcher, IERC20DispatcherTrait};
use dojo_erc::tests::test_erc20_utils::{
    NAME, SYMBOL, DECIMALS, OWNER, SPENDER, SUPPLY, RECIPIENT, VALUE, ZERO, deploy_default
};
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

const INFINITE_ALLOWANCE: u128 = 0xffffffffffffffffffffffffffffffff;

#[test]
#[available_gas(200000000)]
fn test_constructor() {
    let (world, erc20) = deploy_default();
    assert(erc20.balance_of(OWNER()) == SUPPLY, 'Should eq inital_supply');
    assert(erc20.total_supply() == SUPPLY, 'Should eq inital_supply');
    assert(erc20.name() == NAME, 'Name Should be NAME');
    assert(erc20.symbol() == SYMBOL, 'Symbol Should be SYMBOL');
    assert(erc20.decimals() == DECIMALS, 'Decimals Should be 18');
}

#[test]
#[available_gas(200000000)]
fn test_allowance() {
    let (world, erc20) = deploy_default();
    set_contract_address(OWNER());
    erc20.approve(SPENDER(), VALUE);
    assert(erc20.allowance(OWNER(), SPENDER()) == VALUE, 'Should eq VALUE');
}

#[test]
#[available_gas(200000000)]
fn test_approve() {
    let (world, erc20) = deploy_default();
    set_contract_address(OWNER());
    assert(erc20.approve(SPENDER(), VALUE), 'Should return true');
    assert(erc20.allowance(OWNER(), SPENDER()) == VALUE, 'Spender not approved correctly');
}

#[test]
#[available_gas(200000000)]
#[should_panic(
    expected: (
        'ERC20: approve from 0',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED'
    )
)]
fn test_approve_from_zero() {
    let (world, erc20) = deploy_default();
    set_contract_address(ZERO());
    erc20.approve(SPENDER(), VALUE);
}

#[test]
#[available_gas(200000000)]
#[should_panic(
    expected: (
        'ERC20: approve to 0',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED'
    )
)]
fn test_approve_to_zero() {
    set_contract_address(OWNER());
    let (world, erc20) = deploy_default();
    erc20.approve(Zeroable::zero(), VALUE);
}

#[test]
#[available_gas(200000000)]
fn test_transfer() {
    let (world, erc20) = deploy_default();
    set_contract_address(OWNER());
    assert(erc20.transfer(RECIPIENT(), VALUE), 'Should return true');
}

#[test]
#[available_gas(2000000000)]
#[should_panic(
    expected: (
        'u128_sub Overflow',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED'
    )
)]
fn test_transfer_not_enough_balance() {
    let (world, erc20) = deploy_default();
    set_contract_address(OWNER());
    let balance_plus_one = SUPPLY + 1;
    erc20.transfer(RECIPIENT(), balance_plus_one.into());
}

#[test]
#[available_gas(2000000000)]
#[should_panic(
    expected: (
        'ERC20: transfer from 0',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED'
    )
)]
fn test_transfer_from_zero() {
    let (world, erc20) = deploy_default();
    set_contract_address(ZERO());
    erc20.transfer(RECIPIENT(), VALUE);
}

#[test]
#[available_gas(2000000000)]
#[should_panic(
    expected: (
        'ERC20: transfer to 0',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED'
    )
)]
fn test_transfer_to_zero() {
    let (world, erc20) = deploy_default();
    set_contract_address(RECIPIENT());
    erc20.transfer(Zeroable::zero(), VALUE);
}

#[test]
#[available_gas(200000000)]
fn test_transfer_from() {
    let (world, erc20) = deploy_default();
    set_contract_address(OWNER());
    erc20.approve(SPENDER(), VALUE);

    set_contract_address(SPENDER());
    assert(erc20.transfer_from(OWNER(), RECIPIENT(), VALUE), 'Should return true');
    assert(erc20.balance_of(RECIPIENT()) == VALUE, 'Should eq amount');
    assert(erc20.balance_of(OWNER()) == (SUPPLY - VALUE).into(), 'Should eq suppy - amount');
    assert(erc20.allowance(OWNER(), SPENDER()) == 0.into(), 'Should eq to 0');
    assert(erc20.total_supply() == SUPPLY, 'Total supply should not change');
}

#[test]
#[available_gas(200000000)]
fn test_transfer_from_doesnt_consume_infinite_allowance() {
    let (world, erc20) = deploy_default();
    set_contract_address(OWNER());
    erc20.approve(SPENDER(), INFINITE_ALLOWANCE.into());

    set_contract_address(SPENDER());
    assert(erc20.transfer_from(OWNER(), RECIPIENT(), VALUE), 'Should return true');
    assert(
        erc20.allowance(OWNER(), SPENDER()) == INFINITE_ALLOWANCE.into(),
        'allowance should not change'
    );
}

#[test]
#[available_gas(200000000)]
#[should_panic(
    expected: (
        'u128_sub Overflow',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED'
    )
)]
fn test_transfer_from_greater_than_allowance() {
    let (world, erc20) = deploy_default();
    set_contract_address(OWNER());
    erc20.approve(SPENDER(), VALUE);

    set_contract_address(SPENDER());
    let allowance_plus_one = VALUE + 1;
    erc20.transfer_from(OWNER(), RECIPIENT(), allowance_plus_one);
}

#[test]
#[available_gas(200000000)]
#[should_panic(
    expected: (
        'ERC20: transfer to 0',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED'
    )
)]
fn test_transfer_from_to_zero_address() {
    let (world, erc20) = deploy_default();
    set_contract_address(OWNER());
    erc20.approve(SPENDER(), VALUE);

    set_contract_address(SPENDER());
    erc20.transfer_from(OWNER(), Zeroable::zero(), VALUE);
}

#[test]
#[available_gas(200000000)]
#[should_panic(
    expected: (
        'ERC20: transfer from 0',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED',
        'ENTRYPOINT_FAILED'
    )
)]
fn test_transfer_from_from_zero_address() {
    let (world, erc20) = deploy_default();
    erc20.transfer_from(Zeroable::zero(), RECIPIENT(), VALUE);
}

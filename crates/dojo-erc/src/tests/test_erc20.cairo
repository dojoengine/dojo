use array::ArrayTrait;
use debug::PrintTrait;
use option::OptionTrait;
use result::ResultTrait;
use starknet::{ContractAddress, contract_address_const};
use starknet::class_hash::ClassHash;
use starknet::class_hash::Felt252TryIntoClassHash;
use starknet::syscalls::deploy_syscall;
use starknet::SyscallResultTrait;
use traits::{Into, TryInto};

use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
use dojo_erc::erc20::erc20::{IERC20Dispatcher, IERC20DispatcherTrait};
use dojo_erc::tests::test_utils::{NAME, SYMBOL, DECIMALS, SUPPLY, RECIPIENT, deploy_erc20};

const VALUE: felt252 = 300;

fn WORLD() -> ContractAddress {
    contract_address_const::<0x1>()
}
fn OWNER() -> ContractAddress {
    contract_address_const::<0x4>()
}
fn SPENDER() -> ContractAddress {
    contract_address_const::<0x5>()
}

#[test]
#[available_gas(200000000)]
fn test_constructor() {
    let (world, erc20_address) = deploy_erc20();
    assert(
        IERC20Dispatcher {
            contract_address: erc20_address
        }.balance_of(RECIPIENT()) == SUPPLY.into(),
        'Should eq inital_supply'
    );
    assert(
        IERC20Dispatcher {
            contract_address: erc20_address
        }.total_supply() == SUPPLY.into(),
        'Should eq inital_supply'
    );
    assert(
        IERC20Dispatcher {
            contract_address: erc20_address
        }.name() == NAME,
        'Name Should be NAME'
    );
    assert(
        IERC20Dispatcher {
            contract_address: erc20_address
        }.symbol() == SYMBOL,
        'Symbol Should be SYMBOL'
    );
    assert(
        IERC20Dispatcher {
            contract_address: erc20_address
        }.decimals() == DECIMALS,
        'Decimals Should be 18'
    );
    
}


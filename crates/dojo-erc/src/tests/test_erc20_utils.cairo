use array::{ArrayTrait, SpanTrait};
use core::debug::PrintTrait;
use option::OptionTrait;
use result::ResultTrait;
use starknet::SyscallResultTrait;
use starknet::{
    ClassHash, ContractAddress, syscalls::deploy_syscall, class_hash::Felt252TryIntoClassHash,
    get_caller_address, contract_address_const
};
use traits::{Into, TryInto};

use dojo::executor::executor;
use dojo::test_utils::spawn_test_world;
use dojo::world::{world, IWorldDispatcher, IWorldDispatcherTrait};
use dojo_erc::erc20::components::{allowance, balance, supply};
use dojo_erc::erc20::erc20::ERC20;
use dojo_erc::erc20::systems::{erc20_approve, erc20_burn, erc20_mint, erc20_transfer_from};

const DECIMALS: u8 = 18;
const NAME: felt252 = 111;
const SUPPLY: u256 = 2000;
const SYMBOL: felt252 = 222;
const VALUE: u256 = 300;

fn OWNER() -> ContractAddress {
    contract_address_const::<0x5>()
}
fn RECIPIENT() -> ContractAddress {
    contract_address_const::<0x7>()
}
fn SPENDER() -> ContractAddress {
    contract_address_const::<0x6>()
}

fn deploy_erc20() -> (IWorldDispatcher, ContractAddress) {
    let mut systems = array![];
    systems.append(erc20_approve::TEST_CLASS_HASH);
    systems.append(erc20_burn::TEST_CLASS_HASH);
    systems.append(erc20_mint::TEST_CLASS_HASH);
    systems.append(erc20_transfer_from::TEST_CLASS_HASH);

    let mut components = array![];
    components.append(allowance::TEST_CLASS_HASH);
    components.append(balance::TEST_CLASS_HASH);
    components.append(supply::TEST_CLASS_HASH);
    let world = spawn_test_world(components, systems);
    let mut calldata: Array<felt252> = array![];
    calldata.append(world.contract_address.into());
    calldata.append(NAME);
    calldata.append(SYMBOL);
    calldata.append(DECIMALS.into());
    calldata.append(SUPPLY.try_into().unwrap());
    calldata.append(OWNER().into());
    let (erc20_address, _) = deploy_syscall(
        ERC20::TEST_CLASS_HASH.try_into().unwrap(), 0, calldata.span(), false
    )
        .unwrap_syscall();
    return (world, erc20_address);
}

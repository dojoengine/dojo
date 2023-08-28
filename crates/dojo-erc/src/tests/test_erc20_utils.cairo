use array::{ArrayTrait, SpanTrait};
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
use dojo_erc::erc20::systems::{
    erc20_approve, erc20_burn, erc20_mint, erc20_transfer_from, erc20_increase_allowance,
    erc20_decrease_allowance
};
use dojo_erc::erc20::interface::{IERC20Dispatcher, IERC20DispatcherTrait};

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

fn deploy_erc20() -> (IWorldDispatcher, IERC20Dispatcher) {
    let mut systems = array![
        erc20_approve::TEST_CLASS_HASH,
        erc20_burn::TEST_CLASS_HASH,
        erc20_mint::TEST_CLASS_HASH,
        erc20_transfer_from::TEST_CLASS_HASH,
        erc20_increase_allowance::TEST_CLASS_HASH,
        erc20_decrease_allowance::TEST_CLASS_HASH
    ];

    let mut components = array![
        allowance::TEST_CLASS_HASH, balance::TEST_CLASS_HASH, supply::TEST_CLASS_HASH
    ];
    let world = spawn_test_world(components, systems);

    let mut calldata: Array<felt252> = array![
        world.contract_address.into(),
        NAME,
        SYMBOL,
        DECIMALS.into(),
        SUPPLY.try_into().unwrap(),
        OWNER().into()
    ];
    let (erc20_address, _) = deploy_syscall(
        ERC20::TEST_CLASS_HASH.try_into().unwrap(), 0, calldata.span(), false
    )
        .unwrap_syscall();
    return (world, IERC20Dispatcher { contract_address: erc20_address });
}

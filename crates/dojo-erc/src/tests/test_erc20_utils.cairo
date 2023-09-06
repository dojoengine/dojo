use traits::{Into, TryInto};
use option::{Option, OptionTrait};
use result::ResultTrait;
use array::ArrayTrait;
use starknet::SyscallResultTrait;
use starknet::{ContractAddress, contract_address_const};
use starknet::syscalls::deploy_syscall;
use starknet::testing::set_contract_address;

use dojo::executor::executor;
use dojo::test_utils::spawn_test_world;
use dojo::world::{world, IWorldDispatcher, IWorldDispatcherTrait};
use dojo_erc::erc20::components::{erc_20_allowance, erc_20_balance, erc_20_supply};
use dojo_erc::erc20::erc20::ERC20;
use dojo_erc::erc20::systems::{
    ERC20Approve, ERC20DecreaseAllowance, ERC20IncreaseAllowance, ERC20TransferFrom, ERC20Burn,
    ERC20Mint,
};
use dojo_erc::erc20::interface::{IERC20Dispatcher, IERC20DispatcherTrait};
use dojo_erc::tests::test_utils::impersonate;

const DECIMALS: u8 = 18;
const NAME: felt252 = 111;
const SUPPLY: u256 = 2000;
const SYMBOL: felt252 = 222;
const VALUE: u256 = 300;

fn DEPLOYER() -> ContractAddress {
    starknet::contract_address_const::<0x420>()
}

fn OWNER() -> ContractAddress {
    contract_address_const::<0x5>()
}
fn RECIPIENT() -> ContractAddress {
    contract_address_const::<0x7>()
}
fn SPENDER() -> ContractAddress {
    contract_address_const::<0x6>()
}
fn ZERO() -> ContractAddress {
    starknet::contract_address_const::<0x0>()
}

fn spawn_world(world_admin: ContractAddress) -> IWorldDispatcher {
    impersonate(world_admin);

    // components
    let mut components = array![
        erc_20_allowance::TEST_CLASS_HASH,
        erc_20_balance::TEST_CLASS_HASH,
        erc_20_supply::TEST_CLASS_HASH
    ];

    // systems
    let mut systems = array![
        ERC20Approve::TEST_CLASS_HASH,
        ERC20DecreaseAllowance::TEST_CLASS_HASH,
        ERC20IncreaseAllowance::TEST_CLASS_HASH,
        ERC20TransferFrom::TEST_CLASS_HASH,
        ERC20Burn::TEST_CLASS_HASH,
        ERC20Mint::TEST_CLASS_HASH,
    ];

    let world = spawn_test_world(components, systems);

    // Grants writer rights for Component / System

    // erc_20_allowance
    world.grant_writer('ERC20Allowance', 'ERC20Approve');
    world.grant_writer('ERC20Allowance', 'ERC20IncreaseAllowance');
    world.grant_writer('ERC20Allowance', 'ERC20DecreaseAllowance');

    // erc_20_balance
    world.grant_writer('ERC20Balance', 'ERC20TransferFrom');
    world.grant_writer('ERC20Balance', 'ERC20Mint');

    world
}

fn deploy_erc20(
    world: IWorldDispatcher,
    name: felt252,
    symbol: felt252,
    decimals: u8,
    initial_supply: u256,
    recipient: ContractAddress,
) -> ContractAddress {
    let constructor_calldata: Array<felt252> = array![
        world.contract_address.into(),
        name,
        symbol,
        decimals.into(),
        initial_supply.try_into().unwrap(),
        recipient.into(),
    ];
    let (deployed_address, _) = deploy_syscall(
        ERC20::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    )
        .unwrap_syscall();

    deployed_address
}

fn deploy_default() -> (IWorldDispatcher, IERC20Dispatcher) {
    let world = spawn_world(DEPLOYER());
    let contract_address = deploy_erc20(world, NAME, SYMBOL, DECIMALS, SUPPLY, OWNER());
    let erc20 = IERC20Dispatcher { contract_address };

    (world, erc20)
}


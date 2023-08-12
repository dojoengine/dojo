use starknet::{
    ClassHash, ContractAddress, syscalls::deploy_syscall, class_hash::Felt252TryIntoClassHash,
    get_caller_address, contract_address_const
};
use array::{ArrayTrait, SpanTrait};
use traits::{Into, TryInto};
use option::OptionTrait;
use result::ResultTrait;
use starknet::SyscallResultTrait;

use dojo_erc::erc20::components::{balance, supply};
use dojo_erc::erc20::erc20::ERC20;
use dojo_erc::erc20::systems::{erc20_mint};
use dojo::executor::executor;
use dojo::world::{world, IWorldDispatcher, IWorldDispatcherTrait};

fn spawn_test_world(
    components: Array<felt252>, systems: Array<felt252>
) -> (IWorldDispatcher, ContractAddress) {
    // deploy executor
    let constructor_calldata = array::ArrayTrait::new();
    let (executor_address, _) = deploy_syscall(
        executor::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
    )
        .unwrap();

    // deploy world
    let mut world_constructor_calldata = array::ArrayTrait::new();
    world_constructor_calldata.append(executor_address.into());
    let (world_address, _) = deploy_syscall(
        world::TEST_CLASS_HASH.try_into().unwrap(), 0, world_constructor_calldata.span(), false
    )
        .unwrap();
    let world = IWorldDispatcher { contract_address: world_address };

    // register components
    let mut index = 0;
    loop {
        if index == components.len() {
            break ();
        }
        world.register_component((*components[index]).try_into().unwrap());
        index += 1;
    };

    // register systems
    let mut index = 0;
    loop {
        if index == systems.len() {
            break ();
        }
        world.register_system((*systems[index]).try_into().unwrap());
        index += 1;
    };

    (world, world_address)
}

const NAME: felt252 = 111;
const SYMBOL: felt252 = 222;
const DECIMALS: u8 = 18;
const SUPPLY: felt252 = 2000;

fn RECIPIENT() -> ContractAddress {
    contract_address_const::<0x6>()
}

fn deploy_erc20() -> (IWorldDispatcher, ContractAddress) {
    let mut systems = array![];
    // systems.append(erc20_spawn::TEST_CLASS_HASH);
    systems.append(erc20_mint::TEST_CLASS_HASH);

    let mut components = array![];
    components.append(balance::TEST_CLASS_HASH);
    components.append(supply::TEST_CLASS_HASH);
    let (world, world_address) = spawn_test_world(components, systems);
    let mut calldata: Array<felt252> = array![];
    calldata.append(world_address.into());
    calldata.append(NAME);
    calldata.append(SYMBOL);
    calldata.append(DECIMALS.into());
    calldata.append(SUPPLY);
    calldata.append(RECIPIENT().into());
    let (erc20_address, _) = deploy_syscall(
        ERC20::TEST_CLASS_HASH.try_into().unwrap(), 0, calldata.span(), false
    )
        .unwrap_syscall();
    return (world, erc20_address);
}

use core::zeroable::Zeroable;
use core::traits::{Into, Default};
use array::ArrayTrait;
use serde::Serde;
use starknet::ContractAddress;
use starknet::testing::set_contract_address;

use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo_erc::tests::test_erc1155_utils::{
    spawn_world, deploy_erc1155, deploy_default, USER1, USER2, DEPLOYER
};
use dojo_erc::erc1155::interface::{IERC1155, IERC1155Dispatcher, IERC1155DispatcherTrait};

#[test]
#[available_gas(30000000)]
fn test_deploy() {
    let world = spawn_world();
    let erc1155_address = deploy_erc1155(world, DEPLOYER(), 'uri', 'seed-42');
    let erc1155 = IERC1155Dispatcher { contract_address: erc1155_address };
    assert(erc1155.owner() == DEPLOYER(), 'invalid owner');
}

#[test]
#[available_gas(30000000)]
fn test_deploy_default() {
    let (world, erc1155) = deploy_default();
    assert(erc1155.owner() == DEPLOYER(), 'invalid owner');
}
 
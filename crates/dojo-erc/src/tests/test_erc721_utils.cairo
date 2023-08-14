use traits::{Into, TryInto, Default};
use option::{Option, OptionTrait};
use result::ResultTrait;
use array::ArrayTrait;
use serde::Serde;

use starknet::ContractAddress;
use starknet::syscalls::deploy_syscall;
use starknet::testing::set_contract_address;

use debug::PrintTrait;

use dojo::test_utils::spawn_test_world;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo_erc::erc721::erc721::ERC721;
use dojo_erc::erc721::interface::{IERC721, IERC721Dispatcher, IERC721DispatcherTrait};

use dojo_erc::erc721::components::{
    balance, Balance, owner, Owner, token_approval, TokenApproval, operator_approval,
    OperatorApproval, token_uri, TokenUri
};
use dojo_erc::erc721::systems::{
    erc721_approve, erc721_set_approval_for_all, erc721_transfer_from, erc721_mint, erc721_burn,
};


fn spawn_world() -> IWorldDispatcher {
    // components
    let mut components = array![
        balance::TEST_CLASS_HASH,
        owner::TEST_CLASS_HASH,
        token_approval::TEST_CLASS_HASH,
        operator_approval::TEST_CLASS_HASH,
        token_uri::TEST_CLASS_HASH,
    ];

    // systems
    let mut systems = array![
        erc721_approve::TEST_CLASS_HASH,
        erc721_set_approval_for_all::TEST_CLASS_HASH,
        erc721_transfer_from::TEST_CLASS_HASH,
        erc721_mint::TEST_CLASS_HASH,
        erc721_burn::TEST_CLASS_HASH,
    ];

    let world = spawn_test_world(components, systems);
    world
}


fn deploy_erc721(
    world: IWorldDispatcher,
    deployer: ContractAddress,
    name: felt252,
    symbol: felt252,
    uri: felt252,
    seed: felt252
) -> ContractAddress {
    let world = spawn_world();

    let constructor_calldata = array![
        world.contract_address.into(), deployer.into(), name, symbol, uri
    ];
    let (deployed_address, _) = deploy_syscall(
        ERC721::TEST_CLASS_HASH.try_into().unwrap(), seed, constructor_calldata.span(), false
    )
        .expect('error deploying');
    //.unwrap();

    deployed_address
}


fn deploy_default() -> (IWorldDispatcher, IERC721Dispatcher) {
    let deployer = starknet::contract_address_const::<0x420>();

    let world = spawn_world();
    let erc721_address = deploy_erc721(world, deployer, 'name', 'symbol', 'uri', 'seed-42');
    let erc721 = IERC721Dispatcher { contract_address: erc721_address };

    (world, erc721)
}

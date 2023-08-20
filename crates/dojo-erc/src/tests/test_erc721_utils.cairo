use traits::{Into, TryInto};
use option::{Option, OptionTrait};
use result::ResultTrait;
use array::ArrayTrait;

use starknet::ContractAddress;
use starknet::syscalls::deploy_syscall;
use starknet::testing::set_contract_address;

use dojo::test_utils::spawn_test_world;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo_erc::erc721::erc721::ERC721;
use dojo_erc::erc721::interface::{IERC721, IERC721ADispatcher, IERC721ADispatcherTrait};

use dojo_erc::erc721::components::{
    erc_721_balance, erc_721_owner, erc_721_token_approval, operator_approval, base_uri
};
use dojo_erc::erc721::systems::{
    ERC721Approve, ERC721SetApprovalForAll, ERC721TransferFrom, ERC721Mint, ERC721Burn,
    ERC721SetBaseUri
};

fn DEPLOYER() -> ContractAddress {
    starknet::contract_address_const::<0x420>()
}

fn USER1() -> ContractAddress {
    starknet::contract_address_const::<0x111>()
}

fn USER2() -> ContractAddress {
    starknet::contract_address_const::<0x222>()
}

fn ZERO() -> ContractAddress {
    starknet::contract_address_const::<0x0>()
}

fn PROXY() -> ContractAddress {
    starknet::contract_address_const::<0x999>()
}

fn spawn_world() -> IWorldDispatcher {
    // components
    let mut components = array![
        erc_721_balance::TEST_CLASS_HASH,
        erc_721_owner::TEST_CLASS_HASH,
        erc_721_token_approval::TEST_CLASS_HASH,
        operator_approval::TEST_CLASS_HASH,
        base_uri::TEST_CLASS_HASH,
    ];

    // systems
    let mut systems = array![
        ERC721Approve::TEST_CLASS_HASH,
        ERC721SetApprovalForAll::TEST_CLASS_HASH,
        ERC721TransferFrom::TEST_CLASS_HASH,
        ERC721Mint::TEST_CLASS_HASH,
        ERC721Burn::TEST_CLASS_HASH,
        ERC721SetBaseUri::TEST_CLASS_HASH,
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
    let constructor_calldata = array![
        world.contract_address.into(), deployer.into(), name, symbol, uri
    ];
    let (deployed_address, _) = deploy_syscall(
        ERC721::TEST_CLASS_HASH.try_into().unwrap(), seed, constructor_calldata.span(), false
    )
        .expect('error deploying ERC721');

    deployed_address
}


fn deploy_default() -> (IWorldDispatcher, IERC721ADispatcher) {
    let world = spawn_world();
    let erc721_address = deploy_erc721(world, DEPLOYER(), 'name', 'symbol', 'uri', 'seed-42');
    let erc721 = IERC721ADispatcher { contract_address: erc721_address };

    (world, erc721)
}

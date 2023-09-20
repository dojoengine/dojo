use traits::{Into, TryInto};
use option::{Option, OptionTrait};
use result::ResultTrait;
use array::ArrayTrait;

use starknet::{ContractAddress, SyscallResultTrait};
use starknet::syscalls::deploy_syscall;
use starknet::testing::set_contract_address;

use dojo::test_utils::spawn_test_world;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};
use dojo_erc::tests::test_utils::impersonate;

use dojo_erc::erc721::erc721::ERC721;
use dojo_erc::erc721::interface::{
    IERC721, IERC721ADispatcher, IERC721ADispatcherTrait, IERC721CustomDispatcher,
    IERC721CustomDispatcherTrait
};

use dojo_erc::erc721::components::{
    erc_721_balance, erc_721_owner, erc_721_token_approval, operator_approval, base_uri
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

fn USER3() -> ContractAddress {
    starknet::contract_address_const::<0x333>()
}

fn ZERO() -> ContractAddress {
    starknet::contract_address_const::<0x0>()
}

fn PROXY() -> ContractAddress {
    starknet::contract_address_const::<0x999>()
}

fn spawn_world(world_admin: ContractAddress) -> IWorldDispatcher {
    impersonate(world_admin);

    // components
    let mut components = array![
        erc_721_balance::TEST_CLASS_HASH,
        erc_721_owner::TEST_CLASS_HASH,
        erc_721_token_approval::TEST_CLASS_HASH,
        operator_approval::TEST_CLASS_HASH,
        base_uri::TEST_CLASS_HASH,
    ];

    // systems - ERC721 system is registered later
    let mut systems = array![];

    spawn_test_world(components, systems)
}


fn deploy_erc721(
    world: IWorldDispatcher,
    deployer: ContractAddress,
    name: felt252,
    symbol: felt252,
    uri: felt252,
    seed: felt252
) -> ContractAddress {
    let constructor_calldata = array![deployer.into(), name, symbol,];
    let (contract_address, _) = deploy_syscall(
        ERC721::TEST_CLASS_HASH.try_into().unwrap(), seed, constructor_calldata.span(), false
    )
        .unwrap_syscall();
    let erc721_custom = IERC721CustomDispatcher { contract_address };

    // Add ERC721 system and grant writer rights
    world.register_system_contract('ERC721', contract_address);
    //  erc_721_balance
    world.grant_writer('ERC721Balance', 'ERC721');
    // erc_721_owner
    world.grant_writer('ERC721Owner', 'ERC721');
    // erc_721_token_approval
    world.grant_writer('ERC721TokenApproval', 'ERC721');
    // operator_approval
    world.grant_writer('OperatorApproval', 'ERC721');
    // base_uri
    world.grant_writer('BaseUri', 'ERC721');

    erc721_custom.init_world(world.contract_address, uri);

    contract_address
}


fn deploy_default() -> (IWorldDispatcher, IERC721ADispatcher) {
    let world = spawn_world(DEPLOYER());
    let erc721_address = deploy_erc721(world, DEPLOYER(), 'name', 'symbol', 'uri', 'seed-42');
    let erc721 = IERC721ADispatcher { contract_address: erc721_address };

    (world, erc721)
}


fn deploy_testcase1() -> (IWorldDispatcher, IERC721ADispatcher) {
    let world = spawn_world(DEPLOYER());
    let erc721_address = deploy_erc721(world, DEPLOYER(), 'name', 'symbol', 'uri', 'seed-42');
    let erc721 = IERC721ADispatcher { contract_address: erc721_address };

    // user1 owns id : 1,2,3
    erc721.mint(USER1(), 1);
    erc721.mint(USER1(), 2);
    erc721.mint(USER1(), 3);

    // proxy owns id : 10, 11,12,13
    erc721.mint(PROXY(), 10);
    erc721.mint(PROXY(), 11);
    erc721.mint(PROXY(), 12);
    erc721.mint(PROXY(), 13);

    //user2 owns id : 20
    erc721.mint(USER2(), 20);

    impersonate(USER1());
    //user1 approve_for_all proxy
    erc721.set_approval_for_all(PROXY(), true);

    (world, erc721)
}


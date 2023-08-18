use traits::{Into, TryInto};
use option::{Option, OptionTrait};
use result::ResultTrait;
use array::ArrayTrait;

use starknet::ContractAddress;
use starknet::syscalls::deploy_syscall;
use starknet::testing::set_contract_address;

use dojo::test_utils::spawn_test_world;
use dojo::world::{IWorldDispatcher, IWorldDispatcherTrait};

use dojo_erc::erc1155::erc1155::ERC1155;

use dojo_erc::erc1155::components::{erc_1155_balance, uri, operator_approval};
use dojo_erc::erc1155::systems::{
    ERC1155SetApprovalForAll, ERC1155SetUri, ERC1155SafeTransferFrom, ERC1155SafeBatchTransferFrom,
    ERC1155Mint, ERC1155Burn
};


// TODO: move

#[starknet::interface]
trait IERC1155<TState> {
    fn safe_batch_transfer_from(
        ref self: TState,
        from: ContractAddress,
        to: ContractAddress,
        ids: Array<u256>,
        amounts: Array<u256>,
        data: Array<u8>
    );
    fn safe_transfer_from(
        ref self: TState,
        from: ContractAddress,
        to: ContractAddress,
        id: u256,
        amount: u256,
        data: Array<u8>
    );
    fn set_approval_for_all(ref self: TState, operator: ContractAddress, approved: bool);
    fn is_approved_for_all(
        self: @TState, account: ContractAddress, operator: ContractAddress
    ) -> bool;
    fn balance_of(self: @TState, account: ContractAddress, id: u256) -> u256;
    fn balance_of_batch(
        self: @TState, accounts: Array<ContractAddress>, ids: Array<u256>
    ) -> Array<u256>;
    fn uri(self: @TState, token_id: u256) -> felt252;
    fn supports_interface(self: @TState, interface_id: u32) -> bool;

    // Test methods
    fn owner(self: @TState) -> ContractAddress;
    fn mint(ref self: TState, to: ContractAddress, id: felt252, amount: u128, data: Array<u8>);
    fn burn(ref self: TState, from: ContractAddress, id: felt252, amount: u128);
}

fn ZERO() -> ContractAddress {
    starknet::contract_address_const::<0x0>()
}

fn DEPLOYER() -> ContractAddress {
    starknet::contract_address_const::<0x420>()
}

fn USER1() -> ContractAddress {
    starknet::contract_address_const::<0x111>()
}

fn USER2() -> ContractAddress {
    starknet::contract_address_const::<0x222>()
}

fn PROXY() -> ContractAddress {
    starknet::contract_address_const::<0x999>()
}

fn spawn_world() -> IWorldDispatcher {
    // components
    let mut components = array![
        erc_1155_balance::TEST_CLASS_HASH, uri::TEST_CLASS_HASH, operator_approval::TEST_CLASS_HASH, 
    ];

    // systems
    let mut systems = array![
        ERC1155SetApprovalForAll::TEST_CLASS_HASH,
        ERC1155SetUri::TEST_CLASS_HASH,
        ERC1155SafeTransferFrom::TEST_CLASS_HASH,
        ERC1155SafeBatchTransferFrom::TEST_CLASS_HASH,
        ERC1155Mint::TEST_CLASS_HASH,
        ERC1155Burn::TEST_CLASS_HASH,
    ];

    let world = spawn_test_world(components, systems);
    world
}

fn deploy_erc1155(
    world: IWorldDispatcher, deployer: ContractAddress, uri: felt252, seed: felt252
) -> ContractAddress {
    let constructor_calldata = array![world.contract_address.into(), deployer.into(), uri];
    let (deployed_address, _) = deploy_syscall(
        ERC1155::TEST_CLASS_HASH.try_into().unwrap(), seed, constructor_calldata.span(), false
    )
        .expect('error deploying ERC1155');

    deployed_address
}


fn deploy_default() -> (IWorldDispatcher, IERC1155Dispatcher) {
    let world = spawn_world();
    let erc1155_address = deploy_erc1155(world, DEPLOYER(), 'uri', 'seed-42');
    let erc1155 = IERC1155Dispatcher { contract_address: erc1155_address };

    (world, erc1155)
}


fn deploy_testcase1() -> (IWorldDispatcher, IERC1155Dispatcher) {
    let world = spawn_world();
    let erc1155_address = deploy_erc1155(world, DEPLOYER(), 'uri', 'seed-42');
    let erc1155 = IERC1155Dispatcher { contract_address: erc1155_address };

    // proxy  token_id 1  x 5
    erc1155.mint(PROXY(), 1, 5, array![]);
    // proxy  token_id 2 x 5
    erc1155.mint(PROXY(), 2, 5, array![]);
    // proxy  token_id 3 x 5
    erc1155.mint(PROXY(), 3, 5, array![]);

    // user1  token_id 1  x 10
    erc1155.mint(USER1(), 1, 10, array![]);
    // user1  token_id 2 x 20
    erc1155.mint(USER1(), 2, 20, array![]);
    // user1  token_id 3 x 30
    erc1155.mint(USER1(), 3, 30, array![]);

    set_contract_address(USER1());
    //user1 approve_for_all proxy
    erc1155.set_approval_for_all(PROXY(), true);

    (world, erc1155)
}

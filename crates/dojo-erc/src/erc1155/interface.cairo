use starknet::ContractAddress;
use dojo_erc::erc165::interface::{IERC165};

// ERC165
const IERC1155_ID: u32 = 0xd9b67a26_u32;
const IERC1155_METADATA_ID: u32 = 0x0e89341c_u32;
const IERC1155_RECEIVER_ID: u32 = 0x4e2312e0_u32;
const ON_ERC1155_RECEIVED_SELECTOR: u32 = 0xf23a6e61_u32;
const ON_ERC1155_BATCH_RECEIVED_SELECTOR: u32 = 0xbc197c81_u32;


// full interface IERC165 + IERC1155 + IERC1155Metadata + custom
#[starknet::interface]
trait IERC1155A<TState> {
    // IERC165
    fn supports_interface(self: @TState, interface_id: u32) -> bool;

    // IERC1155
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

    // IERC1155Metadata
    fn uri(self: @TState, token_id: u256) -> felt252;

    // custom
    fn owner(self: @TState) -> ContractAddress;
    fn mint(ref self: TState, to: ContractAddress, id: felt252, amount: u128, data: Array<u8>);
    fn burn(ref self: TState, from: ContractAddress, id: felt252, amount: u128);
}

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
}

#[starknet::interface]
trait IERC1155Metadata<TState> {
    fn uri(self: @TState, token_id: u256) -> felt252;
}

#[starknet::interface]
trait IERC1155TokenReceiver<TState> {
    fn on_erc1155_received(
        self: TState,
        operator: ContractAddress,
        from: ContractAddress,
        token_id: u256,
        amount: u256,
        data: Array<u8>
    ) -> u32;
    fn on_erc1155_batch_received(
        self: TState,
        operator: ContractAddress,
        from: ContractAddress,
        token_ids: Array<u256>,
        amounts: Array<u256>,
        data: Array<u8>
    ) -> u32;
}


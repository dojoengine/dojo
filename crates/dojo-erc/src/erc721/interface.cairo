use starknet::ContractAddress;

// ERC165
const IERC721_ID: u32 = 0x80ac58cd_u32;
const IERC721_METADATA_ID: u32 = 0x5b5e139f_u32;
const IERC721_RECEIVER_ID: u32 = 0x150b7a02_u32;
const IERC721_ENUMERABLE_ID: u32 = 0x780e9d63_u32;


// full interface IERC165 + IERC721 + IERC721Metadata + custom
#[starknet::interface]
trait IERC721A<TState> {
    // IERC165
    fn supports_interface(self: @TState, interface_id: u32) -> bool;

    // IERC721
    fn balance_of(self: @TState, account: ContractAddress) -> u256;
    fn owner_of(self: @TState, token_id: u256) -> ContractAddress;

    fn approve(ref self: TState, to: ContractAddress, token_id: u256);
    fn get_approved(self: @TState, token_id: u256) -> ContractAddress;
    fn is_approved_for_all(
        self: @TState, owner: ContractAddress, operator: ContractAddress
    ) -> bool;
    fn set_approval_for_all(ref self: TState, operator: ContractAddress, approved: bool);

    fn transfer_from(ref self: TState, from: ContractAddress, to: ContractAddress, token_id: u256);
    fn safe_transfer_from(
        ref self: TState,
        from: ContractAddress,
        to: ContractAddress,
        token_id: u256,
        data: Span<felt252>
    );

    // IERC721Metadata
    fn name(self: @TState) -> felt252;
    fn symbol(self: @TState) -> felt252;
    fn token_uri(self: @TState, token_id: u256) -> felt252;

    // custom
    fn owner(self: @TState) -> ContractAddress;
    fn mint(ref self: TState, to: ContractAddress, token_id: u256);
    fn burn(ref self: TState, token_id: u256);
    fn exists(self: @TState, token_id: u256) -> bool;
    fn transfer(ref self: TState, to: ContractAddress, token_id: u256);
}


#[starknet::interface]
trait IERC721<TState> {
    fn balance_of(self: @TState, account: ContractAddress) -> u256;
    fn owner_of(self: @TState, token_id: u256) -> ContractAddress;

    fn approve(ref self: TState, to: ContractAddress, token_id: u256);
    fn get_approved(self: @TState, token_id: u256) -> ContractAddress;

    fn is_approved_for_all(
        self: @TState, owner: ContractAddress, operator: ContractAddress
    ) -> bool;
    fn set_approval_for_all(ref self: TState, operator: ContractAddress, approved: bool);

    fn transfer_from(ref self: TState, from: ContractAddress, to: ContractAddress, token_id: u256);
    fn safe_transfer_from(
        ref self: TState,
        from: ContractAddress,
        to: ContractAddress,
        token_id: u256,
        data: Span<felt252>
    );
}


#[starknet::interface]
trait IERC721Metadata<TState> {
    fn name(self: @TState) -> felt252;
    fn symbol(self: @TState) -> felt252;
    fn token_uri(self: @TState, token_id: u256) -> felt252;
}

#[starknet::interface]
trait IERC721Custom<TState> {
    fn init_world(ref self: TState, world: ContractAddress, uri: felt252);

    fn exists(self: @TState, token_id: u256) -> bool;

    fn owner(self: @TState) -> ContractAddress;

    fn transfer(ref self: TState, to: ContractAddress, token_id: u256);

    fn mint(ref self: TState, to: ContractAddress, token_id: u256);

    fn burn(ref self: TState, token_id: u256);
}

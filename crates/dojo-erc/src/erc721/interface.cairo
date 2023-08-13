use starknet::ContractAddress;

#[starknet::interface]
trait IERC721<TState> {
    fn owner(self: @TState) -> ContractAddress;
    fn name(self: @TState) -> felt252;
    fn symbol(self: @TState) -> felt252;
    fn token_uri(self: @TState, token_id: u256) -> felt252;

    fn balance_of(self: @TState, account: ContractAddress) -> u256;
    fn owner_of(self: @TState, token_id: u256) -> ContractAddress;

    fn get_approved(self: @TState, token_id: u256) -> ContractAddress;
    fn is_approved_for_all(
        self: @TState, owner: ContractAddress, operator: ContractAddress
    ) -> bool;
    fn approve(ref self: TState, to: ContractAddress, token_id: u256);
    fn set_approval_for_all(ref self: TState, operator: ContractAddress, approved: bool);

    fn transfer(ref self: TState, to: ContractAddress, token_id: u256);
    fn transfer_from(ref self: TState, from: ContractAddress, to: ContractAddress, token_id: u256);
    fn safe_transfer_from(
        ref self: TState,
        from: ContractAddress,
        to: ContractAddress,
        token_id: u256,
        data: Span<felt252>
    );

    fn mint(ref self: TState, to: ContractAddress, token_id: u256);
    fn burn(ref self: TState, token_id: u256);

    fn exists(self: @TState, token_id: u256) -> bool;
}


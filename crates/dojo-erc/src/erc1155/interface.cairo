use starknet::ContractAddress;

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

    //
    fn owner(self: @TState) -> ContractAddress;
    fn mint(ref self: TState, to: ContractAddress, id: u256, amount: u256, data: Array<u8>);

    fn mint_batch(
        ref self: TState,
        to: ContractAddress,
        ids: Array<u256>,
        amounts: Array<u256>,
        data: Array<u8>
    );
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

#[starknet::interface]
trait IERC165<TState> {
    fn supports_interface(self: TState, interface_id: u32) -> bool;
}

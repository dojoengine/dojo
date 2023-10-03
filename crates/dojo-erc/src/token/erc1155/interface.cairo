use starknet::ContractAddress;

#[starknet::interface]
trait IERC1155<TState> {
    fn balance_of(self: @TState, account: ContractAddress, id: u256) -> u256;
    fn balance_of_batch(
        self: @TState, accounts: Array<ContractAddress>, ids: Array<u256>
    ) -> Array<u256>;
    fn set_approval_for_all(ref self: TState, operator: ContractAddress, approved: bool);
    fn is_approved_for_all(
        self: @TState, account: ContractAddress, operator: ContractAddress
    ) -> bool;
    fn safe_transfer_from(
        ref self: TState,
        from: ContractAddress,
        to: ContractAddress,
        id: u256,
        amount: u256,
        data: Array<u8>
    );
    fn safe_batch_transfer_from(
        ref self: TState,
        from: ContractAddress,
        to: ContractAddress,
        ids: Array<u256>,
        amounts: Array<u256>,
        data: Array<u8>
    );
}

#[starknet::interface]
trait IERC1155CamelOnly<TState> {
    fn balanceOf(self: @TState, account: ContractAddress, id: u256) -> u256;
    fn balanceOfBatch(
        self: @TState, accounts: Array<ContractAddress>, ids: Array<u256>
    ) -> Array<u256>;
    fn setApprovalForAll(ref self: TState, operator: ContractAddress, approved: bool);
    fn isApprovedForAll(self: @TState, account: ContractAddress, operator: ContractAddress) -> bool;
    fn safeTransferFrom(
        ref self: TState,
        from: ContractAddress,
        to: ContractAddress,
        id: u256,
        amount: u256,
        data: Array<u8>
    );
    fn safeBatchTransferFrom(
        ref self: TState,
        from: ContractAddress,
        to: ContractAddress,
        ids: Array<u256>,
        amounts: Array<u256>,
        data: Array<u8>
    );
}

#[starknet::interface]
trait IERC1155Metadata<TState> {
    fn name(self: @TState) -> felt252;
    fn symbol(self: @TState) -> felt252;
    fn uri(self: @TState, token_id: u256) -> felt252;
}

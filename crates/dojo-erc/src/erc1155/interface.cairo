use starknet::ContractAddress;

#[starknet::interface]
trait IERC1155<TContractState> {
    fn safe_batch_transfer_from(
        ref self: TContractState,
        from: ContractAddress,
        to: ContractAddress,
        ids: Array<u256>,
        amounts: Array<u256>,
        data: Array<u8>
    );
    fn safe_transfer_from(
        ref self: TContractState,
        from: ContractAddress,
        to: ContractAddress,
        id: u256,
        amount: u256,
        data: Array<u8>
    );
    fn set_approval_for_all(ref self: TContractState, operator: ContractAddress, approved: bool);
    fn is_approved_for_all(
        self: @TContractState, account: ContractAddress, operator: ContractAddress
    ) -> bool;
    fn balance_of(self: @TContractState, account: ContractAddress, id: u256) -> u256;
    fn balance_of_batch(
        self: @TContractState, accounts: Array<ContractAddress>, ids: Array<u256>
    ) -> Array<u256>;
    fn uri(self: @TContractState, token_id: u256) -> felt252;
    fn supports_interface(self: @TContractState, interface_id: u32) -> bool;
}

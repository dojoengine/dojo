use cairo_verifier::StarkProofWithSerde;
use starknet::ContractAddress;

#[starknet::interface]
trait IUpgradeableState<TContractState> {
    fn upgrade_state(ref self: TContractState, new_state: Span<felt252>, program_output: Span<felt252>);
}

#[starknet::interface]
trait IFactRegistry<TContractState> {
    fn is_valid(self: @TContractState, fact: felt252) -> bool;
}

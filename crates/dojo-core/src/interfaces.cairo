use starknet::ContractAddress;

#[derive(Drop, Serde)]
struct StorageUpdate {
    key: felt252,
    value: felt252,
}

#[derive(Drop, Serde)]
struct ProgramOutput {
    prev_state_root: felt252,
    new_state_root: felt252,
    block_number: felt252,
    block_hash: felt252,
    config_hash: felt252,
    world_da_hash: felt252,
    message_to_starknet_segment: Span<felt252>,
    message_to_appchain_segment: Span<felt252>,
}

#[starknet::interface]
trait IUpgradeableState<TContractState> {
    fn upgrade_state(
        ref self: TContractState, new_state: Span<StorageUpdate>, program_output: ProgramOutput
    );
}

#[starknet::interface]
trait IFactRegistry<TContractState> {
    fn is_valid(self: @TContractState, fact: felt252) -> bool;
}

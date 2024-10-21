use starknet::ContractAddress;

#[derive(Drop, Serde)]
pub struct StorageUpdate {
    pub key: felt252,
    pub value: felt252,
}

#[derive(Drop, Serde)]
pub struct ProgramOutput {
    pub prev_state_root: felt252,
    pub new_state_root: felt252,
    pub block_number: felt252,
    pub block_hash: felt252,
    pub config_hash: felt252,
    pub world_da_hash: felt252,
    pub message_to_starknet_segment: Span<felt252>,
    pub message_to_appchain_segment: Span<felt252>,
}

#[starknet::interface]
pub trait IUpgradeableState<TContractState> {
    fn upgrade_state(
        ref self: TContractState,
        new_state: Span<StorageUpdate>,
        program_output: ProgramOutput,
        program_hash: felt252
    );
}

#[starknet::interface]
pub trait IFactRegistry<TContractState> {
    fn is_valid(self: @TContractState, fact: felt252) -> bool;
}


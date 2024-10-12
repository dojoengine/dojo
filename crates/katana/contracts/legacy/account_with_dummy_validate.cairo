#[starknet::contract]
mod Account {
    use array::{ArrayTrait, SpanTrait};
    use starknet::{ContractAddress, call_contract_syscall};
    use starknet::info::SyscallResultTrait;
    use zeroable::Zeroable;

    #[storage]
    struct Storage {
    }

    #[external(v0)]
    fn __validate_deploy__(
        self: @ContractState,
        class_hash: felt252,
        contract_address_salt: felt252
    ) -> felt252 {
        starknet::VALIDATED
    }

    #[external(v0)]
    fn __validate_declare__(self: @ContractState, class_hash: felt252) -> felt252 {
        starknet::VALIDATED
    }

    #[external(v0)]
    fn __validate__(
        self: @ContractState,
        contract_address: ContractAddress,
        selector: felt252,
        calldata: Array<felt252>
    ) -> felt252 {
        starknet::VALIDATED
    }

    #[external(v0)]
    #[raw_output]
    fn __execute__(
        self: @ContractState,
        contract_address: ContractAddress,
        selector: felt252,
        calldata: Array<felt252>
    ) -> Span<felt252> {
        // Validate caller.
        assert(starknet::get_caller_address().is_zero(), 'INVALID_CALLER');

        call_contract_syscall(
            address: contract_address,
            entry_point_selector: selector,
            calldata: calldata.span()
        ).unwrap_syscall()
    }
}

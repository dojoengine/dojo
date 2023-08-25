use starknet::ClassHash;

use dojo::world::Context;

#[starknet::interface]
trait IExecutor<T> {
    fn execute(self: @T, class_hash: ClassHash, calldata: Span<felt252>) -> Span<felt252>;
    fn call(
        self: @T, class_hash: ClassHash, entrypoint: felt252, calldata: Span<felt252>
    ) -> Span<felt252>;
}

#[starknet::contract]
mod executor {
    use array::{ArrayTrait, SpanTrait};
    use option::OptionTrait;
    use starknet::ClassHash;

    use super::IExecutor;

    const EXECUTE_ENTRYPOINT: felt252 =
        0x0240060cdb34fcc260f41eac7474ee1d7c80b7e3607daff9ac67c7ea2ebb1c44;

    const WORLD_ADDRESS_OFFSET: u32 = 4;

    #[storage]
    struct Storage {}

    #[external(v0)]
    impl Executor of IExecutor<ContractState> {
        /// Executes a System by calling its execute entrypoint.
        ///
        /// # Arguments
        ///
        /// * `class_hash` - Class Hash of the System.
        /// * `calldata` - Calldata to pass to the System.
        ///
        /// # Returns
        ///
        /// The return value of the System's execute entrypoint.
        fn execute(
            self: @ContractState, class_hash: ClassHash, calldata: Span<felt252>
        ) -> Span<felt252> {
            assert(
                traits::Into::<starknet::ContractAddress,
                felt252>::into(starknet::get_caller_address()) == *calldata
                    .at(calldata.len() - WORLD_ADDRESS_OFFSET),
                'Only world caller'
            );
            starknet::syscalls::library_call_syscall(class_hash, EXECUTE_ENTRYPOINT, calldata)
                .unwrap_syscall()
        }

        /// Call the provided `entrypoint` method on the given `class_hash`.
        ///
        /// # Arguments
        ///
        /// * `class_hash` - Class Hash to call.
        /// * `entrypoint` - Entrypoint to call.
        /// * `calldata` - The calldata to pass.
        ///
        /// # Returns
        ///
        /// The return value of the call.
        fn call(
            self: @ContractState,
            class_hash: ClassHash,
            entrypoint: felt252,
            calldata: Span<felt252>
        ) -> Span<felt252> {
            starknet::syscalls::library_call_syscall(class_hash, entrypoint, calldata)
                .unwrap_syscall()
        }
    }
}

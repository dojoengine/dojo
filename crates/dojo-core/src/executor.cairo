use starknet::ClassHash;

use dojo::world::Context;

#[starknet::interface]
trait IExecutor<T> {
    fn execute(self: @T, class_hash: ClassHash, calldata: Span<felt252>) -> Span<felt252>;
}

#[starknet::contract]
mod executor {
    use array::{ArrayTrait, SpanTrait};
    use option::OptionTrait;
    use starknet::ClassHash;

    use super::IExecutor;

    const EXECUTE_ENTRYPOINT: felt252 =
        0x0240060cdb34fcc260f41eac7474ee1d7c80b7e3607daff9ac67c7ea2ebb1c44;

    #[storage]
    struct Storage {}

    #[external(v0)]
    impl Executor of IExecutor<ContractState> {
        /// Executes a System by calling its execute entrypoint.
        ///
        /// # Arguments
        ///
        /// * `calldata` - The calldata to pass to the System.
        ///
        /// # Returns
        ///
        /// The return value of the System's execute entrypoint.
        fn execute(
            self: @ContractState, class_hash: ClassHash, calldata: Span<felt252>
        ) -> Span<felt252> {
            starknet::syscalls::library_call_syscall(class_hash, EXECUTE_ENTRYPOINT, calldata)
                .unwrap_syscall()
        }
    }
}

use starknet::ClassHash;

#[starknet::interface]
trait IExecutor<T> {
    fn call(
        self: @T, class_hash: ClassHash, entrypoint: felt252, calldata: Span<felt252>
    ) -> Span<felt252>;
}

#[starknet::contract]
mod executor {
    use array::{ArrayTrait, SpanTrait};
    use option::OptionTrait;
    use starknet::{ClassHash, SyscallResultTrait, SyscallResultTraitImpl};

    use super::IExecutor;

    #[storage]
    struct Storage {}

    #[external(v0)]
    impl Executor of IExecutor<ContractState> {
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

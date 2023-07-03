use dojo::world::Context;

#[starknet::interface]
trait IExecutor<T> {
    fn execute(self: @T, ctx: Context, calldata: Span<felt252>) -> Span<felt252>;
}

#[starknet::contract]
mod executor {
    use array::{ArrayTrait, ArrayTCloneImpl, SpanTrait};
    use serde::Serde;
    use clone::Clone;
    use box::BoxTrait;
    use traits::{TryInto, Into};
    use option::OptionTrait;
    use starknet::{get_caller_address, get_tx_info};

    use dojo::world::IWorldDispatcher;
    use dojo::interfaces::{ISystemLibraryDispatcher, ISystemDispatcherTrait};
    use dojo::world::Context;

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
        /// * `ctx` - The world's context for the execution.
        /// * `calldata` - The calldata to pass to the System.
        ///
        /// # Returns
        ///
        /// The return value of the System's execute entrypoint.
        fn execute(
            self: @ContractState, ctx: Context, mut calldata: Span<felt252>
        ) -> Span<felt252> {
            // Get the world address and instantiate the world dispatcher.
            let world_address = get_caller_address();
            let world = IWorldDispatcher { contract_address: world_address };

            // Serialize the context
            let mut calldata_arr = ArrayTrait::new();
            ctx.serialize(ref calldata_arr);

            // Append the calldata
            loop {
                match calldata.pop_front() {
                    Option::Some(val) => {
                        calldata_arr.append(*val);
                    },
                    Option::None(_) => {
                        break ();
                    }
                };
            };

            // Call the system
            let res = starknet::syscalls::library_call_syscall(
                ctx.system_class_hash, EXECUTE_ENTRYPOINT, calldata_arr.span()
            )
                .unwrap_syscall();

            res
        }
    }
}

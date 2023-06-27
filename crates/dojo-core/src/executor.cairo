#[starknet::contract]
mod Executor {
    use array::{ArrayTrait, ArrayTCloneImpl, SpanTrait};
    use serde::Serde;
    use clone::Clone;
    use box::BoxTrait;
    use traits::Into;
    use dojo::execution_context::Context;
    use dojo::interfaces::{IWorldDispatcher, ISystemLibraryDispatcher, ISystemDispatcherTrait};
    use dojo::auth::components::AuthRole;
    use starknet::contract_address::ContractAddressIntoFelt252;
    use starknet::{get_caller_address, get_tx_info};

    const EXECUTE_ENTRYPOINT: felt252 =
        0x0240060cdb34fcc260f41eac7474ee1d7c80b7e3607daff9ac67c7ea2ebb1c44;

    #[storage]
    struct Storage {}

    /// Executes a System by calling its execute entrypoint.
    ///
    /// # Arguments
    ///
    /// * `class_hash` - The class hash of the System to execute.
    /// * `execution_role` - The execution role to be assumed by the System.
    /// * `execute_calldata` - The calldata to pass to the System.
    ///
    /// # Returns
    ///
    /// The return value of the System's execute entrypoint.
    #[external(v0)]
    fn execute(
        self: @ContractState,
        class_hash: starknet::ClassHash,
        execution_role: AuthRole,
        mut execute_calldata: Span<felt252>
    ) -> Span<felt252> {
        // Get the world address and instantiate the world dispatcher.
        let world_address = get_caller_address();
        let world = IWorldDispatcher { contract_address: world_address };

        // Get the caller account address
        let caller_account = get_tx_info().unbox().account_contract_address;

        // Get system name
        let caller_system = ISystemLibraryDispatcher { class_hash }.name();

        // Instantiate the execution context
        let mut ctx = Context { world, caller_account, caller_system, execution_role,  };

        // Serialize the context
        let mut calldata_arr = ArrayTrait::new();
        ctx.serialize(ref calldata_arr);

        // Append the execute_calldata
        loop {
            match execute_calldata.pop_front() {
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
            class_hash, EXECUTE_ENTRYPOINT, calldata_arr.span()
        )
            .unwrap_syscall();
        res
    }
}

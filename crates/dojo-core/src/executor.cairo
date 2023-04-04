#[contract]
mod Executor {
    use array::ArrayTrait;
    use traits::Into;
    use starknet::contract_address::ContractAddressIntoFelt252;

    use dojo_core::serde::SpanSerde;

    const EXECUTE_ENTRYPOINT: felt252 =
        0x240060cdb34fcc260f41eac7474ee1d7c80b7e3607daff9ac67c7ea2ebb1c44;

    #[external]
    #[raw_output]
    fn execute(class_hash: starknet::ClassHash, mut calldata: Span<felt252>, ) -> Span<felt252> {
        let world_address = starknet::get_caller_address();

        // TODO: use span pop_back to mutate input calldata once it is available.
        let mut calldata_arr = ArrayTrait::new();
        array::clone_loop(calldata, ref calldata_arr);
        calldata_arr.append(world_address.into());

        let res = starknet::syscalls::library_call_syscall(
            class_hash, EXECUTE_ENTRYPOINT, calldata_arr.span()
        ).unwrap_syscall();
        res
    }
}

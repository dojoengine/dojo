#[contract]
mod Executor {
    use array::{ArrayTrait, ArrayTCloneImpl};
    use clone::Clone;
    use traits::Into;
    use starknet::contract_address::ContractAddressIntoFelt252;

    use dojo_core::serde::SpanSerde;

    const EXECUTE_ENTRYPOINT: felt252 =
        0x0240060cdb34fcc260f41eac7474ee1d7c80b7e3607daff9ac67c7ea2ebb1c44;

    #[external]
    fn execute(class_hash: starknet::ClassHash, calldata: Span<felt252>) -> Span<felt252> {
        let world_address = starknet::get_caller_address();

        let mut calldata_arr = calldata.snapshot.clone();
        calldata_arr.append(world_address.into());

        let res = starknet::syscalls::library_call_syscall(
            class_hash, EXECUTE_ENTRYPOINT, calldata_arr.span()
        ).unwrap_syscall();
        res
    }
}

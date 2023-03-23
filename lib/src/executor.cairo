#[abi]
trait IExecutor {
    fn execute(
        class_hash: starknet::ClassHash,
        world_address: starknet::ContractAddress,
        data: Span<felt252>
    ) -> Span<felt252>;
}

#[contract]
mod Executor {
    use dojo::serde::SpanSerde;

    const EXECUTE_ENTRYPOINT: felt252 =
        0x240060cdb34fcc260f41eac7474ee1d7c80b7e3607daff9ac67c7ea2ebb1c44;

    #[external]
    #[raw_output]
    fn execute(
        class_hash: starknet::ClassHash,
        world_address: starknet::ContractAddress,
        data: Span<felt252>,
    ) -> Span<felt252> {
        // TODO: Pass world_address to system. Do we need to clone the calldata array or is there a better
        // approach?
        let res = starknet::syscalls::library_call_syscall(
            class_hash, EXECUTE_ENTRYPOINT, data
        ).unwrap_syscall();
        res
    }
}

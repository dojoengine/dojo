#[abi]
trait IExecutor {
    fn execute(
        world: starknet::ContractAddress, class_hash: starknet::ClassHash, data: Span<felt252>
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
        world: starknet::ContractAddress, class_hash: starknet::ClassHash, data: Span<felt252>
    ) -> Span<felt252> {
        let res = starknet::syscalls::library_call_syscall(
            class_hash, EXECUTE_ENTRYPOINT, data
        ).unwrap_syscall();
        res
    }
}

#[contract]
mod Executor {
    use array::ArrayTrait;
    use array::ArrayTCloneImpl;
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

mod tests {
    use core::traits::Into;
    use core::result::ResultTrait;
    use array::ArrayTrait;
    use option::OptionTrait;
    use traits::TryInto;

    use starknet::syscalls::deploy_syscall;
    use starknet::class_hash::Felt252TryIntoClassHash;
    use dojo_core::interfaces::IExecutorDispatcher;
    use dojo_core::interfaces::IExecutorDispatcherTrait;

    #[derive(Component)]
    struct Foo {
        a: felt252,
        b: u128,
    }

    #[system]
    mod Bar {
        use super::Foo;

        fn execute(foo: Foo) -> Foo {
            foo
        }
    }

    #[test]
    #[available_gas(30000000)]
    fn test_executor() {
        let constructor_calldata = array::ArrayTrait::<felt252>::new();
        let (executor_address, _) = deploy_syscall(
            super::Executor::TEST_CLASS_HASH.try_into().unwrap(), 0, constructor_calldata.span(), false
        ).unwrap();

        let executor = IExecutorDispatcher { contract_address: executor_address };

        let mut system_calldata = ArrayTrait::new();
        system_calldata.append(42);
        system_calldata.append(53);
        let res = executor.execute(BarSystem::TEST_CLASS_HASH.try_into().unwrap(), system_calldata.span());
    }
}

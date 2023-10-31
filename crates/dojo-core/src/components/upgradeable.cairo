use starknet::ClassHash;

#[starknet::interface]
trait IUpgradeable<T> {
    fn upgrade(ref self: T, new_class_hash: ClassHash);
}

#[starknet::component]
mod upgradeable {
    use starknet::ClassHash;
    use starknet::ContractAddress;
    use starknet::get_contract_address;
    use starknet::syscalls::replace_class_syscall;

    #[storage]
    struct Storage {}

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {}

    mod Errors {
        const INVALID_CLASS: felt252 = 'class_hash cannot be zero';
    }

    #[generate_trait]
    impl InternalImpl<
        TContractState, +HasComponent<TContractState>
    > of InternalTrait<TContractState> {
        fn upgrade(ref self: ComponentState<TContractState>, new_class_hash: ClassHash) {
            assert(new_class_hash.is_non_zero(), Errors::INVALID_CLASS);
            replace_class_syscall(new_class_hash).unwrap();
        }
    }
}

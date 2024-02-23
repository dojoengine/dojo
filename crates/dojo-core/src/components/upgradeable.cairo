use starknet::ClassHash;

#[starknet::interface]
trait IUpgradeable<T> {
    fn upgrade(ref self: T, new_class_hash: ClassHash);
}

#[starknet::component]
mod upgradeable {
    use core::starknet::SyscallResultTrait;
    use starknet::ClassHash;
    use starknet::ContractAddress;
    use starknet::get_caller_address;
    use starknet::syscalls::replace_class_syscall;
    use dojo::world::{IWorldProvider, IWorldProviderDispatcher, IWorldDispatcher};

    #[storage]
    struct Storage {}

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        Upgraded: Upgraded,
    }

    #[derive(Drop, starknet::Event)]
    struct Upgraded {
        class_hash: ClassHash
    }

    mod Errors {
        const INVALID_CLASS: felt252 = 'class_hash cannot be zero';
        const INVALID_CLASS_CONTENT: felt252 = 'class_hash not world provider';
        const INVALID_CALLER: felt252 = 'must be called by world';
        const INVALID_WORLD_ADDRESS: felt252 = 'invalid world address';
    }

    use debug::PrintTrait;

    #[embeddable_as(UpgradableImpl)]
    impl Upgradable<
        TContractState, +HasComponent<TContractState>, +IWorldProvider<TContractState>
    > of super::IUpgradeable<ComponentState<TContractState>> {
        fn upgrade(ref self: ComponentState<TContractState>, new_class_hash: ClassHash) {
            assert(
                self.get_contract().world().contract_address.is_non_zero(),
                Errors::INVALID_WORLD_ADDRESS
            );
            assert(
                get_caller_address() == self.get_contract().world().contract_address,
                Errors::INVALID_CALLER
            );
            assert(new_class_hash.is_non_zero(), Errors::INVALID_CLASS);

            match starknet::library_call_syscall(
                new_class_hash,
                selector!("world"),
                array![].span(),
            ) {
                Result::Ok(_) => {
                    replace_class_syscall(new_class_hash).unwrap();
                    self.emit(Upgraded { class_hash: new_class_hash });
                },
                Result::Err(_) => panic_with_felt252(Errors::INVALID_CLASS_CONTENT),
            }
        }
    }
}

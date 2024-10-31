use starknet::ClassHash;

#[starknet::interface]
pub trait IUpgradeable<T> {
    fn upgrade(ref self: T, new_class_hash: ClassHash);
}

#[starknet::component]
pub mod upgradeable_cpt {
    use core::num::traits::Zero;

    use starknet::{ClassHash, get_caller_address};
    use starknet::syscalls::replace_class_syscall;

    use dojo::contract::components::world_provider::IWorldProvider;

    #[storage]
    pub struct Storage {}

    #[event]
    #[derive(Drop, starknet::Event)]
    pub enum Event {
        Upgraded: Upgraded,
    }

    #[derive(Drop, starknet::Event)]
    pub struct Upgraded {
        pub class_hash: ClassHash
    }

    pub mod Errors {
        pub const INVALID_CLASS: felt252 = 'class_hash cannot be zero';
        pub const INVALID_CLASS_CONTENT: felt252 = 'class_hash not Dojo IContract';
        pub const INVALID_CALLER: felt252 = 'must be called by world';
        pub const INVALID_WORLD_ADDRESS: felt252 = 'invalid world address';
    }

    #[embeddable_as(UpgradeableImpl)]
    impl Upgradeable<
        TContractState, +HasComponent<TContractState>, +IWorldProvider<TContractState>
    > of super::IUpgradeable<ComponentState<TContractState>> {
        fn upgrade(ref self: ComponentState<TContractState>, new_class_hash: ClassHash) {
            assert(
                self.get_contract().world_dispatcher().contract_address.is_non_zero(),
                Errors::INVALID_WORLD_ADDRESS
            );
            assert(
                get_caller_address() == self.get_contract().world_dispatcher().contract_address,
                Errors::INVALID_CALLER
            );
            assert(new_class_hash.is_non_zero(), Errors::INVALID_CLASS);

            // Seems like the match doesn't catch the error is the entrypoint is
            // not found.
            match starknet::syscalls::library_call_syscall(
                new_class_hash, selector!("dojo_name"), [].span(),
            ) {
                Result::Ok(_) => {
                    replace_class_syscall(new_class_hash).unwrap();
                    self.emit(Upgraded { class_hash: new_class_hash });
                },
                Result::Err(_) => core::panic_with_felt252(Errors::INVALID_CLASS_CONTENT),
            }
        }
    }
}

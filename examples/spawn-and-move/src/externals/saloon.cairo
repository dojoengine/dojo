#[starknet::contract]
mod Saloon {
    use starknet::storage::{StoragePointerReadAccess, StoragePointerWriteAccess};
    use openzeppelin::upgrades::interface::IUpgradeable;
    use openzeppelin::upgrades::UpgradeableComponent;
    use starknet::{ContractAddress, ClassHash, get_caller_address};

    component!(path: UpgradeableComponent, storage: upgradeable, event: UpgradeableEvent);

    impl UpgradeableInternalImpl = UpgradeableComponent::InternalImpl<ContractState>;

    #[storage]
    struct Storage {
        owner: ContractAddress,
        #[substorage(v0)]
        upgradeable: UpgradeableComponent::Storage,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        #[flat]
        UpgradeableEvent: UpgradeableComponent::Event,
    }

    #[constructor]
    fn constructor(ref self: ContractState) {
        self.owner.write(get_caller_address());
    }

    #[abi(embed_v0)]
    impl UpgradeableImpl of IUpgradeable<ContractState> {
        fn upgrade(ref self: ContractState, new_class_hash: ClassHash) {
            assert(get_caller_address() == self.owner.read(), 'not the owner');
            self.upgradeable.upgrade(new_class_hash);
        }
    }
}

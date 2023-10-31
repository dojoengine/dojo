use dojo::world::IWorldDispatcher;

#[starknet::interface]
trait IBase<T> {
    fn world(self: @T) -> IWorldDispatcher;
}

#[starknet::contract]
mod base {
    use starknet::{ClassHash, get_caller_address};

    use dojo::world::IWorldDispatcher;

    use dojo::components::upgradeable::IUpgradeable;
    use dojo::components::upgradeable::upgradeable as upgradeable_component;

    component!(path: upgradeable_component, storage: upgradeable, event: UpgradeableEvent);

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        UpgradeableEvent: upgradeable_component::Event
    }

    #[storage]
    struct Storage {
        world_dispatcher: IWorldDispatcher,
        #[substorage(v0)]
        upgradeable: upgradeable_component::Storage,
    }

    #[constructor]
    fn constructor(ref self: ContractState) {
        self.world_dispatcher.write(IWorldDispatcher { contract_address: get_caller_address() });
    }

    #[external(v0)]
    fn world(self: @ContractState) -> IWorldDispatcher {
        self.world_dispatcher.read()
    }

    impl UpgradeableInternalImpl = upgradeable_component::InternalImpl<ContractState>;

    #[external(v0)]
    impl Upgradeable of IUpgradeable<ContractState> {
        /// Upgrade contract implementation to new_class_hash
        ///
        /// # Arguments
        ///
        /// * `new_class_hash` - The new implementation class hash.
        fn upgrade(ref self: ContractState, new_class_hash: ClassHash) {
            self.upgradeable.upgrade(new_class_hash);
        }
    }
}

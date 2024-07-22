#[starknet::contract]
pub mod base {
    use starknet::{ClassHash, ContractAddress, get_caller_address};
    use starknet::storage::{StoragePointerReadAccess, StoragePointerWriteAccess};

    use dojo::contract::upgradeable::upgradeable as upgradeable_component;
    use dojo::world::{IWorldProvider, IWorldDispatcher, IWorldDispatcherTrait};

    component!(path: upgradeable_component, storage: upgradeable, event: UpgradeableEvent);

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        #[flat]
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

    #[abi(embed_v0)]
    impl WorldProviderImpl of IWorldProvider<ContractState> {
        fn world(self: @ContractState) -> IWorldDispatcher {
            self.world_dispatcher.read()
        }
    }

    #[abi(embed_v0)]
    impl UpgradableImpl = upgradeable_component::UpgradableImpl<ContractState>;
}

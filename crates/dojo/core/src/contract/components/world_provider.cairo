use dojo::world::IWorldDispatcher;

#[starknet::interface]
pub trait IWorldProvider<T> {
    fn world_dispatcher(self: @T) -> IWorldDispatcher;
}

#[starknet::component]
pub mod world_provider_cpt {
    use dojo::world::IWorldDispatcher;
    use starknet::get_caller_address;
    use starknet::storage::{StoragePointerReadAccess, StoragePointerWriteAccess};

    #[storage]
    pub struct Storage {
        world_dispatcher: IWorldDispatcher,
    }

    #[embeddable_as(WorldProviderImpl)]
    pub impl WorldProvider<
        TContractState, +HasComponent<TContractState>,
    > of super::IWorldProvider<ComponentState<TContractState>> {
        fn world_dispatcher(self: @ComponentState<TContractState>) -> IWorldDispatcher {
            self.world_dispatcher.read()
        }
    }

    #[generate_trait]
    pub impl InternalImpl<
        TContractState, +HasComponent<TContractState>,
    > of InternalTrait<TContractState> {
        fn initializer(ref self: ComponentState<TContractState>) {
            self
                .world_dispatcher
                .write(IWorldDispatcher { contract_address: get_caller_address() });
        }
    }
}

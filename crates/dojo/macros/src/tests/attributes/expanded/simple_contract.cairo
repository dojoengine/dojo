#[starknet::contract]
pub mod simple_contract {
    use dojo::contract::components::world_provider::{
        world_provider_cpt, world_provider_cpt::InternalTrait as WorldProviderInternal,
        IWorldProvider
    };
    use dojo::contract::components::upgradeable::upgradeable_cpt;
    use dojo::contract::IContract;
    use dojo::meta::IDeployedResource;

    component!(path: world_provider_cpt, storage: world_provider, event: WorldProviderEvent);
    component!(path: upgradeable_cpt, storage: upgradeable, event: UpgradeableEvent);

    #[abi(embed_v0)]
    impl WorldProviderImpl = world_provider_cpt::WorldProviderImpl<ContractState>;

    #[abi(embed_v0)]
    impl UpgradeableImpl = upgradeable_cpt::UpgradeableImpl<ContractState>;

    #[abi(embed_v0)]
    pub impl simple_contract__ContractImpl of IContract<ContractState> {}

    #[abi(embed_v0)]
    pub impl simple_contract__DeployedContractImpl of IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "simple_contract"
        }
    }

    #[generate_trait]
    impl simple_contractInternalImpl of simple_contractInternalTrait {
        fn world(
            self: @ContractState, namespace: @ByteArray
        ) -> dojo::world::storage::WorldStorage {
            dojo::world::WorldStorageTrait::new(self.world_provider.world_dispatcher(), namespace)
        }
    }


    #[constructor]
    fn constructor(ref self: ContractState) {
        self.world_provider.initializer();
    }
    #[abi(per_item)]
    #[generate_trait]
    pub impl IDojoInitImpl of IDojoInit {
        #[external(v0)]
        fn dojo_init(self: @ContractState) {
            if starknet::get_caller_address() != self
                .world_provider
                .world_dispatcher()
                .contract_address {
                core::panics::panic_with_byte_array(
                    @format!(
                        "Only the world can init contract `{}`, but caller is `{:?}`",
                        self.dojo_name(),
                        starknet::get_caller_address(),
                    )
                );
            }
        }
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        UpgradeableEvent: upgradeable_cpt::Event,
        WorldProviderEvent: world_provider_cpt::Event,
    }

    #[storage]
    struct Storage {
        #[substorage(v0)]
        upgradeable: upgradeable_cpt::Storage,
        #[substorage(v0)]
        world_provider: world_provider_cpt::Storage,
    }
}

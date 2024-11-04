#[starknet::contract]
pub mod $name$ {
    use dojo::contract::components::world_provider::{world_provider_cpt, world_provider_cpt::InternalTrait as WorldProviderInternal, IWorldProvider};
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
    pub impl $name$__ContractImpl of IContract<ContractState> {}

    #[abi(embed_v0)]
    pub impl $name$__DeployedContractImpl of IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "$name$"
        }
    }

    #[generate_trait]
    impl $name$InternalImpl of $name$InternalTrait {
        fn world(self: @ContractState, namespace: @ByteArray) -> dojo::world::storage::WorldStorage {
            dojo::world::WorldStorageTrait::new(self.world_provider.world_dispatcher(), namespace)
        }
    }

    $body$
}

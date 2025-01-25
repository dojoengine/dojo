#[starknet::contract]
pub mod $name$ {
    use dojo::contract::components::world_provider::{world_provider_cpt, world_provider_cpt::InternalTrait as WorldProviderInternal, IWorldProvider};
    use dojo::contract::ILibrary;
    use dojo::meta::IDeployedResource;

    component!(path: world_provider_cpt, storage: world_provider, event: WorldProviderEvent);
   
    #[abi(embed_v0)]
    impl WorldProviderImpl = world_provider_cpt::WorldProviderImpl<ContractState>;
   
    #[abi(embed_v0)]
    pub impl $name$__LibraryImpl of ILibrary<ContractState> {}


    // TODO: rename impl ??
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

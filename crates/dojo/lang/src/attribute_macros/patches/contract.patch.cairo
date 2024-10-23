#[starknet::contract]
pub mod $name$ {
    use dojo::contract::components::world_provider::{world_provider_cpt, world_provider_cpt::InternalTrait as WorldProviderInternal, IWorldProvider};
    use dojo::contract::components::upgradeable::upgradeable_cpt;
    use dojo::contract::IContract;

    component!(path: world_provider_cpt, storage: world_provider, event: WorldProviderEvent);
    component!(path: upgradeable_cpt, storage: upgradeable, event: UpgradeableEvent);

    #[abi(embed_v0)]
    impl WorldProviderImpl = world_provider_cpt::WorldProviderImpl<ContractState>;
    
    #[abi(embed_v0)]
    impl UpgradeableImpl = upgradeable_cpt::UpgradeableImpl<ContractState>;

    #[abi(embed_v0)]
    pub impl ContractImpl of IContract<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "$name$"
        }
    }

    $body$
}

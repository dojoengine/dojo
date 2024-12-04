#[starknet::contract]
pub mod complex_contract {
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
    pub impl complex_contract__ContractImpl of IContract<ContractState> {}

    #[abi(embed_v0)]
    pub impl complex_contract__DeployedContractImpl of IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "complex_contract"
        }
    }

    #[generate_trait]
    impl complex_contractInternalImpl of complex_contractInternalTrait {
        fn world(
            self: @ContractState, namespace: @ByteArray
        ) -> dojo::world::storage::WorldStorage {
            dojo::world::WorldStorageTrait::new(self.world_provider.world_dispatcher(), namespace)
        }
    }

    use starknet::{ContractAddress, get_caller_address};

    #[derive(Copy, Drop, Serde)]
    #[dojo::event]
    struct MyInit {
        #[key]
        caller: ContractAddress,
        value: u8,
    }

    #[storage]
    struct Storage {
        #[substorage(v0)]
        upgradeable: upgradeable_cpt::Storage,
        #[substorage(v0)]
        world_provider: world_provider_cpt::Storage,
        value: u128
    }

    #[derive(Drop, starknet::Event)]
    pub struct MyEvent {
        #[key]
        pub selector: felt252,
        pub value: u64,
    }

    #[event]
    #[derive(Drop, starknet::Event)]
    enum Event {
        UpgradeableEvent: upgradeable_cpt::Event,
        WorldProviderEvent: world_provider_cpt::Event,
        MyEvent: MyEvent
    }

    #[constructor]
    fn constructor(ref self: ContractState) {
        self.world_provider.initializer();
        self.value.write(12);
    }

    #[abi(per_item)]
    #[generate_trait]
    pub impl IDojoInitImpl of IDojoInit {
        #[external(v0)]
        fn dojo_init(self: @ContractState, value: u8) {
            if starknet::get_caller_address() != self
                .world_provider
                .world_dispatcher()
                .contract_address {
                core::panics::panic_with_byte_array(
                    @format!(
                        "Only the world can init contract `{}`, but caller is `{:?}`",
                        self.dojo_name(),
                        starknet::get_caller_address()
                    )
                );
            }
            let mut world = self.world(@"ns");
            world.emit_event(@MyInit { caller: get_caller_address(), value });
        }
    }
    #[generate_trait]
    impl SelfImpl of SelfTrait {
        fn my_internal_function(self: @ContractState) -> u8 {
            42
        }
    }
}

//! Test some manually expanded code for permissioned contract deployment and resource registration.
//!

#[starknet::contract]
pub mod attacker_contract {
    use dojo::world::IWorldDispatcher;
    use dojo::contract::components::world_provider::IWorldProvider;
    use dojo::contract::IContract;
    use starknet::storage::StoragePointerReadAccess;

    #[storage]
    struct Storage {
        world_dispatcher: IWorldDispatcher,
    }

    #[abi(embed_v0)]
    pub impl ContractImpl of IContract<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "test_1"
        }
    }

    #[abi(embed_v0)]
    impl WorldProviderImpl of IWorldProvider<ContractState> {
        fn world_dispatcher(self: @ContractState) -> IWorldDispatcher {
            self.world_dispatcher.read()
        }
    }
}

#[starknet::contract]
pub mod attacker_model {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DojoModelImpl of dojo::model::IModel<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "foo"
        }

        fn version(self: @ContractState) -> u8 {
            1
        }

        fn unpacked_size(self: @ContractState) -> Option<usize> {
            Option::None
        }

        fn packed_size(self: @ContractState) -> Option<usize> {
            Option::None
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::Layout::Fixed([].span())
        }

        fn schema(self: @ContractState) -> dojo::meta::introspect::Ty {
            dojo::meta::introspect::Ty::Primitive('felt252')
        }

        fn definition(self: @ContractState) -> dojo::model::ModelDef {
            dojo::model::ModelDef {
                name: Self::dojo_name(self),
                version: Self::version(self),
                layout: Self::layout(self),
                schema: Self::schema(self),
                packed_size: Self::packed_size(self),
                unpacked_size: Self::unpacked_size(self),
            }
        }
    }
}

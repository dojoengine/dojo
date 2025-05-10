//! Test some manually expanded code for permissioned contract deployment and resource registration.
//!

#[starknet::contract]
pub mod attacker_contract {
    use dojo::world::IWorldDispatcher;

    #[storage]
    struct Storage {
        world_dispatcher: IWorldDispatcher,
    }

    #[abi(embed_v0)]
    pub impl DojoDeployedModelImpl of dojo::meta::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "test_1"
        }
    }
}

#[starknet::contract]
pub mod attacker_model {
    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DojoDeployedModelImpl of dojo::meta::IDeployedResource<ContractState> {
        fn dojo_name(self: @ContractState) -> ByteArray {
            "foo"
        }
    }

    #[abi(embed_v0)]
    impl DojoStoredModelImpl of dojo::meta::interface::IStoredResource<ContractState> {
        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::meta::Layout::Fixed([].span())
        }

        fn schema(self: @ContractState) -> dojo::meta::introspect::Struct {
            dojo::meta::introspect::Struct { name: 'm1', attrs: [].span(), children: [].span() }
        }
    }

    #[abi(embed_v0)]
    impl DojoModelImpl of dojo::model::IModel<ContractState> {
        fn unpacked_size(self: @ContractState) -> Option<usize> {
            Option::None
        }

        fn packed_size(self: @ContractState) -> Option<usize> {
            Option::None
        }

        fn definition(self: @ContractState) -> dojo::model::ModelDef {
            dojo::model::ModelDef {
                name: DojoDeployedModelImpl::dojo_name(self),
                layout: DojoStoredModelImpl::layout(self),
                schema: DojoStoredModelImpl::schema(self),
                packed_size: Self::packed_size(self),
                unpacked_size: Self::unpacked_size(self),
            }
        }
    }
}

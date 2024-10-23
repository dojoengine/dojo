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
        fn name(self: @ContractState) -> ByteArray {
            "test_1"
        }

        fn namespace(self: @ContractState) -> ByteArray {
            "ns1"
        }

        fn tag(self: @ContractState) -> ByteArray {
            "other tag"
        }

        fn name_hash(self: @ContractState) -> felt252 {
            'name hash'
        }

        fn namespace_hash(self: @ContractState) -> felt252 {
            dojo::utils::bytearray_hash(@"atk")
        }

        fn selector(self: @ContractState) -> felt252 {
            // Targetting a resource that exists in an other namespace.
            selector_from_tag!("dojo-Foo")
        }
    }

    #[abi(embed_v0)]
    impl WorldProviderImpl of IWorldProvider<ContractState> {
        fn world(self: @ContractState) -> IWorldDispatcher {
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
        fn name(self: @ContractState) -> ByteArray {
            "m1"
        }

        fn namespace(self: @ContractState) -> ByteArray {
            "ns1"
        }

        fn tag(self: @ContractState) -> ByteArray {
            "other tag"
        }

        fn version(self: @ContractState) -> u8 {
            1
        }

        fn selector(self: @ContractState) -> felt252 {
            // Targetting a resource that exists in an other namespace.
            selector_from_tag!("dojo-Foo")
        }

        fn name_hash(self: @ContractState) -> felt252 {
            'name hash'
        }

        fn namespace_hash(self: @ContractState) -> felt252 {
            dojo::utils::bytearray_hash(@"atk")
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
                name: Self::name(self),
                namespace: Self::namespace(self),
                version: Self::version(self),
                selector: Self::selector(self),
                name_hash: Self::name_hash(self),
                namespace_hash: Self::namespace_hash(self),
                layout: Self::layout(self),
                schema: Self::schema(self),
                packed_size: Self::packed_size(self),
                unpacked_size: Self::unpacked_size(self),
            }
        }
    }
}

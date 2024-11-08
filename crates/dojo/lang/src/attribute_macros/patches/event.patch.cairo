pub impl $type_name$DojoEventImpl of dojo::event::Event<$type_name$> {
    #[inline(always)]
    fn name() -> ByteArray {
        "$type_name$"
    }

    #[inline(always)]
    fn definition() -> dojo::event::EventDefinition {
        dojo::event::EventDefinition {
            name: Self::name(),
            layout: Self::layout(),
            schema: Self::schema()
        }
    }

    #[inline(always)]
    fn layout() -> dojo::meta::Layout {
        dojo::meta::introspect::Introspect::<$type_name$>::layout()
    }

    #[inline(always)]
    fn schema() -> dojo::meta::introspect::Struct {
        if let dojo::meta::introspect::Ty::Struct(s) = dojo::meta::introspect::Introspect::<$type_name$>::ty() {
            s
        }
        else {
            panic!("Event `$type_name$`: invalid schema.")
        }
    }

    #[inline(always)]
    fn serialized_keys(self: @$type_name$) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        $serialized_keys$
        core::array::ArrayTrait::span(@serialized)
    }

    #[inline(always)]
    fn serialized_values(self: @$type_name$) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        $serialized_values$
        core::array::ArrayTrait::span(@serialized)
    }

    #[inline(always)]
    fn selector(namespace_hash: felt252) -> felt252 {
        dojo::utils::selector_from_namespace_and_name(namespace_hash, @Self::name())
    }
}

#[starknet::contract]
pub mod e_$type_name$ {
    use super::$type_name$;

    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl $type_name$__DeployedEventImpl = dojo::event::component::IDeployedEventImpl<ContractState, $type_name$>;

    #[abi(embed_v0)]
    impl $type_name$__StoredEventImpl = dojo::event::component::IStoredEventImpl<ContractState, $type_name$>;

     #[abi(embed_v0)]
    impl $type_name$__EventImpl = dojo::event::component::IEventImpl<ContractState, $type_name$>;

    #[abi(per_item)]
    #[generate_trait]
    impl $type_name$Impl of I$type_name${
        // Ensures the ABI contains the Event struct, since it's never used
        // by systems directly.
        #[external(v0)]
        fn ensure_abi(self: @ContractState, event: $type_name$) {
            let _event = event;
        }

        // Ensures the generated contract has a unique classhash, using
        // a hardcoded hash computed on event and member names.
        #[external(v0)]
        fn ensure_unique(self: @ContractState) {
            let _hash = $unique_hash$;
        }
    }
}

pub impl $type_name$DojoEventImpl of dojo::event::Event<$type_name$> {
    #[inline(always)]
    fn name() -> ByteArray {
        "$type_name$"
    }

    #[inline(always)]
    fn version() -> u8 {
        $event_version$
    }

    #[inline(always)]
    fn definition() -> dojo::event::EventDefinition {
        dojo::event::EventDefinition {
            name: Self::name(),
            version: Self::version(),
            layout: Self::layout(),
            schema: Self::schema()
        }
    }

    #[inline(always)]
    fn layout() -> dojo::meta::Layout {
        dojo::meta::introspect::Introspect::<$type_name$>::layout()
    }

    #[inline(always)]
    fn schema() -> dojo::meta::introspect::Ty {
        dojo::meta::introspect::Introspect::<$type_name$>::ty()
    }

    #[inline(always)]
    fn historical() -> bool {
        $event_historical$
    }

    #[inline(always)]
    fn keys(self: @$type_name$) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        $serialized_keys$
        core::array::ArrayTrait::span(@serialized)
    }

    #[inline(always)]
    fn values(self: @$type_name$) -> Span<felt252> {
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
    impl $type_name$__DojoEventImpl of dojo::event::IEvent<ContractState>{
        fn dojo_name(self: @ContractState) -> ByteArray {
           "$type_name$"
        }

        fn version(self: @ContractState) -> u8 {
           $event_version$
        }

        fn definition(self: @ContractState) -> dojo::event::EventDefinition {
            dojo::event::Event::<$type_name$>::definition()
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::event::Event::<$type_name$>::layout()
        }

        fn schema(self: @ContractState) -> dojo::meta::introspect::Ty {
            dojo::meta::introspect::Introspect::<$type_name$>::ty()
        }
    }
}

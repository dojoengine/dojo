// EventValue on it's own does nothing since events are always emitted and
// never read from the storage. However, it's required by the ABI to
// ensure that the event definition contains both keys and values easily distinguishable.
// Only derives strictly required traits.
#[derive(Drop, Serde)]
pub struct $type_name$Value {
    $members_values$
}

pub impl $type_name$Definition of dojo::event::EventDefinition<$type_name$>{
    const SELECTOR: felt252 = "$name_hash$";

    #[inline(always)]
    fn name() -> ByteArray {
        "$type_name$"
    }
}

pub impl $type_name$ModelParser of dojo::model::model::ModelParser<$type_name$>{
    fn serialize_keys(self: @$type_name$) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        $serialized_keys$
        core::array::ArrayTrait::span(@serialized)
    }
    fn serialize_values(self: @$type_name$) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        $serialized_values$
        core::array::ArrayTrait::span(@serialized)
    }
}

pub impl $type_name$EventImpl = dojo::event::event::EventImpl<$type_name$>;

#[starknet::contract]
pub mod e_$type_name$ {
    use super::$type_name$;
    use super::$type_name$Value;

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

        // Outputs EventValue to allow a simple diff from the ABI compared to the
        // event to retrieved the keys of an event.
        #[external(v0)]
        fn ensure_values(self: @ContractState, value: $type_name$Value) {
            let _value = value;
        }

        // Ensures the generated contract has a unique classhash, using
        // a hardcoded hash computed on event and member names.
        #[external(v0)]
        fn ensure_unique(self: @ContractState) {
            let _hash = $unique_hash$;
        }
    }
}

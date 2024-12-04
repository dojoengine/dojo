#[derive(Introspect)]
struct ComplexEvent {
    #[key]
    k1: u8,
    #[key]
    k2: u32,
    v1: u256,
    v2: Option<u128>
}

// EventValue on it's own does nothing since events are always emitted and
// never read from the storage. However, it's required by the ABI to
// ensure that the event definition contains both keys and values easily distinguishable.
// Only derives strictly required traits.
#[derive(Drop, Serde)]
pub struct ComplexEventValue {
    pub v1: u256,
    pub v2: Option<u128>,
}

pub impl ComplexEventDefinition of dojo::event::EventDefinition<ComplexEvent> {
    #[inline(always)]
    fn name() -> ByteArray {
        "ComplexEvent"
    }
}

pub impl ComplexEventModelParser of dojo::model::model::ModelParser<ComplexEvent> {
    fn serialize_keys(self: @ComplexEvent) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        core::serde::Serde::serialize(self.k1, ref serialized);
        core::serde::Serde::serialize(self.k2, ref serialized);

        core::array::ArrayTrait::span(@serialized)
    }
    fn serialize_values(self: @ComplexEvent) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        core::serde::Serde::serialize(self.v1, ref serialized);
        core::serde::Serde::serialize(self.v2, ref serialized);

        core::array::ArrayTrait::span(@serialized)
    }
}

pub impl ComplexEventEventImpl = dojo::event::event::EventImpl<ComplexEvent>;

#[starknet::contract]
pub mod e_ComplexEvent {
    use super::ComplexEvent;
    use super::ComplexEventValue;

    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl ComplexEvent__DeployedEventImpl =
        dojo::event::component::IDeployedEventImpl<ContractState, ComplexEvent>;

    #[abi(embed_v0)]
    impl ComplexEvent__StoredEventImpl =
        dojo::event::component::IStoredEventImpl<ContractState, ComplexEvent>;

    #[abi(embed_v0)]
    impl ComplexEvent__EventImpl =
        dojo::event::component::IEventImpl<ContractState, ComplexEvent>;

    #[abi(per_item)]
    #[generate_trait]
    impl ComplexEventImpl of IComplexEvent {
        // Ensures the ABI contains the Event struct, since it's never used
        // by systems directly.
        #[external(v0)]
        fn ensure_abi(self: @ContractState, event: ComplexEvent) {
            let _event = event;
        }

        // Outputs EventValue to allow a simple diff from the ABI compared to the
        // event to retrieved the keys of an event.
        #[external(v0)]
        fn ensure_values(self: @ContractState, value: ComplexEventValue) {
            let _value = value;
        }

        // Ensures the generated contract has a unique classhash, using
        // a hardcoded hash computed on event and member names.
        #[external(v0)]
        fn ensure_unique(self: @ContractState) {}
    }
}

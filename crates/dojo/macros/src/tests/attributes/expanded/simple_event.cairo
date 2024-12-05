#[derive(Introspect)]
struct SimpleEvent {
    #[key]
    k: u32,
    v: u32
}

// EventValue on it's own does nothing since events are always emitted and
// never read from the storage. However, it's required by the ABI to
// ensure that the event definition contains both keys and values easily distinguishable.
// Only derives strictly required traits.
#[derive(Drop, Serde)]
pub struct SimpleEventValue {
    pub v: u32,
}

pub impl SimpleEventDefinition of dojo::event::EventDefinition<SimpleEvent> {
    #[inline(always)]
    fn name() -> ByteArray {
        "SimpleEvent"
    }
}

pub impl SimpleEventModelParser of dojo::model::model::ModelParser<SimpleEvent> {
    fn serialize_keys(self: @SimpleEvent) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        core::serde::Serde::serialize(self.k, ref serialized);

        core::array::ArrayTrait::span(@serialized)
    }
    fn serialize_values(self: @SimpleEvent) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        core::serde::Serde::serialize(self.v, ref serialized);

        core::array::ArrayTrait::span(@serialized)
    }
}

pub impl SimpleEventEventImpl = dojo::event::event::EventImpl<SimpleEvent>;

#[starknet::contract]
pub mod e_SimpleEvent {
    use super::SimpleEvent;
    use super::SimpleEventValue;

    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl SimpleEvent__DeployedEventImpl =
        dojo::event::component::IDeployedEventImpl<ContractState, SimpleEvent>;

    #[abi(embed_v0)]
    impl SimpleEvent__StoredEventImpl =
        dojo::event::component::IStoredEventImpl<ContractState, SimpleEvent>;

    #[abi(embed_v0)]
    impl SimpleEvent__EventImpl =
        dojo::event::component::IEventImpl<ContractState, SimpleEvent>;

    #[abi(per_item)]
    #[generate_trait]
    impl SimpleEventImpl of ISimpleEvent {
        // Ensures the ABI contains the Event struct, since it's never used
        // by systems directly.
        #[external(v0)]
        fn ensure_abi(self: @ContractState, event: SimpleEvent) {
            let _event = event;
        }

        // Outputs EventValue to allow a simple diff from the ABI compared to the
        // event to retrieved the keys of an event.
        #[external(v0)]
        fn ensure_values(self: @ContractState, value: SimpleEventValue) {
            let _value = value;
        }

        // Ensures the generated contract has a unique classhash, using
        // a hardcoded hash computed on event and member names.
        #[external(v0)]
        fn ensure_unique(self: @ContractState) {}
    }
}

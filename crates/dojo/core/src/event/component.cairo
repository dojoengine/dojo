use dojo::event::{Event, EventDef, IEvent};
use dojo::meta::Layout;
use dojo::meta::introspect::Struct;

#[starknet::embeddable]
pub impl IDeployedEventImpl<
    TContractState, E, +Event<E>,
> of dojo::meta::interface::IDeployedResource<TContractState> {
    fn dojo_name(self: @TContractState) -> ByteArray {
        Event::<E>::name()
    }
}

#[starknet::embeddable]
pub impl IStoredEventImpl<
    TContractState, E, +Event<E>,
> of dojo::meta::interface::IStoredResource<TContractState> {
    fn schema(self: @TContractState) -> Struct {
        Event::<E>::schema()
    }

    fn layout(self: @TContractState) -> Layout {
        Event::<E>::layout()
    }
}

#[starknet::embeddable]
pub impl IEventImpl<TContractState, E, +Event<E>> of IEvent<TContractState> {
    fn definition(self: @TContractState) -> EventDef {
        Event::<E>::definition()
    }
}

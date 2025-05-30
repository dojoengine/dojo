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
        // as events use Serde instead of DojoStore,
        // layout definition must remain as before.
        // (enum variant index starts from 0, ...)
        //
        // Note that event layout is not used at the moment,
        // because events are not stored in the world storage
        // but emitted as real Starknet events.
        dojo::meta::layout::build_legacy_layout(Event::<E>::layout())
    }
}

#[starknet::embeddable]
pub impl IEventImpl<TContractState, E, +Event<E>> of IEvent<TContractState> {
    fn definition(self: @TContractState) -> EventDef {
        Event::<E>::definition()
    }
}

#[starknet::interface]
pub trait IEvent<T> {
    fn definition(self: @T) -> super::EventDefinition;
}

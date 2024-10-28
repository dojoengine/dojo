use super::EventDefinition;

#[starknet::interface]
pub trait IEvent<T> {
    fn definition(self: @T) -> EventDefinition;
}

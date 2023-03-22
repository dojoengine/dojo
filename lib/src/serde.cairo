use option::OptionTrait;
use serde::Serde;

impl SpanSerde of Serde::<Span<felt252>> {
    fn serialize(ref serialized: Array<felt252>, mut input: Span<felt252>) {
        array::clone_loop(input, ref serialized);
    }
    fn deserialize(ref serialized: Span<felt252>) -> Option<Span<felt252>> {
        Option::Some(serialized)
    }
}

use array::ArrayTrait;
use array::SpanTrait;
use option::OptionTrait;
use serde::Serde;

impl SpanSerde of Serde::<Span<felt252>> {
    fn serialize(ref serialized: Array<felt252>, mut input: Span<felt252>) {
        loop {
            gas::withdraw_gas().expect('Out of gas');
            match input.pop_front() {
                Option::Some(v) => {
                    serialized.append(*v);
                },
                Option::None(_) => {
                    break ();
                },
            };
        };
    }
    fn deserialize(ref serialized: Span<felt252>) -> Option<Span<felt252>> {
        Option::Some(serialized)
    }
}

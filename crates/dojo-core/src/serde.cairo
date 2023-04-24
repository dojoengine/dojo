use array::ArrayTrait;
use array::SpanTrait;
use option::OptionTrait;
use serde::Serde;

impl SpanSerde of Serde::<Span<felt252>> {
    fn serialize(ref output: Array<felt252>, mut input: Span<felt252>) {
        Serde::<usize>::serialize(ref output, input.len());
        serialize_span_helper(ref output, input);
    }
    fn deserialize(ref serialized: Span<felt252>) -> Option<Span<felt252>> {
        let length = *serialized.pop_front()?;
        let mut arr = ArrayTrait::new();
        deserialize_array_helper(ref serialized, arr, length)
    }
}

fn serialize_span_helper(
    ref output: Array<felt252>, mut input: Span<felt252>
) {
    match input.pop_front() {
        Option::Some(v) => {
            output.append(*v);
            serialize_span_helper(ref output, input);
        },
        Option::None(_) => {},
    }
}

fn deserialize_array_helper(
    ref serialized: Span<felt252>, mut curr_output: Array<felt252>, remaining: felt252
) -> Option<Span<felt252>> {
    if remaining == 0 {
        return Option::Some(curr_output.span());
    }
    curr_output.append(*serialized.pop_front()?);
    deserialize_array_helper(ref serialized, curr_output, remaining - 1)
}

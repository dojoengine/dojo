use array::ArrayTrait;
use array::SpanTrait;
use option::OptionTrait;
use serde::Serde;
use serde::deserialize_array_helper;

impl SpanSerde<T, impl TSerde: Serde<T>, impl TCopy: Copy<T>, impl TDrop: Drop<T>> of Serde::<Span<T>> {
    fn serialize(ref output: Array<felt252>, mut input: Span<T>) {
        Serde::<usize>::serialize(ref output, input.len());
        serialize_span_helper(ref output, input);
    }
    fn deserialize(ref serialized: Span<felt252>) -> Option<Span<T>> {
        let length = *serialized.pop_front()?;
        let mut arr = ArrayTrait::new();
        Option::Some(deserialize_array_helper(ref serialized, arr, length)?.span())
    }
}

fn serialize_span_helper<T, impl TSerde: Serde<T>, impl TCopy: Copy<T>, impl TDrop: Drop<T>>(
    ref output: Array<felt252>, mut input: Span<T>
) {
    match input.pop_front() {
        Option::Some(value) => {
            let value = *value;
            TSerde::serialize(ref output, value);
            serialize_span_helper(ref output, input);
        },
        Option::None(_) => {},
    }
}

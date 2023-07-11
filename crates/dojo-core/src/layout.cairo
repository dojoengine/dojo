use core::traits::Into;
use array::{ArrayTrait, SpanTrait};

struct LayoutItem {
    value: felt252,
    size: u8
}

trait StorageLayoutTrait<T> {
    fn to_layout(self: @T) -> Array<LayoutItem>;
    fn get_layout() -> Span<u8>;
    fn from_unpacked(items: Span<felt252>) -> T;
}

fn layout_length(ref layout: Span<u8>) -> u32 {
    let mut sum: u32 = 0;
    loop {
        match layout.pop_front() {
            Option::Some(i) => {
                sum = sum + (*i).into();
            },
            Option::None(_) => {
                break;
            }
        };
    };
    sum
}

fn unpack(packed: Span<felt252>, layout: Span<u8>) -> Span<felt252> {
    ArrayTrait::new().span()
}

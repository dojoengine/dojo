use dojo::storage::packing::calculate_packed_size;

#[derive(Copy, Drop, Serde, Debug, PartialEq)]
pub struct FieldLayout {
    pub selector: felt252,
    pub layout: Layout
}

#[derive(Copy, Drop, Serde, Debug, PartialEq)]
pub enum Layout {
    Fixed: Span<u8>,
    Struct: Span<FieldLayout>,
    Tuple: Span<Layout>,
    // We can't have `Layout` here as it will cause infinite recursion.
    // And `Box` is not serializable. So using a Span, even if it's to have
    // one element, does the trick.
    Array: Span<Layout>,
    ByteArray,
    // there is one layout per variant.
    // the `selector` field identifies the variant
    // the `layout` defines the variant data (could be empty for variant without data).
    Enum: Span<FieldLayout>,
}

// TODO RBA
pub fn compute_packed_size(layout: Layout) -> Option<usize> {
    if let Layout::Fixed(layout) = layout {
        let mut span_layout = layout;
        Option::Some(calculate_packed_size(ref span_layout))
    } else {
        Option::None
    }
}

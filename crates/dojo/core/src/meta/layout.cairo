use dojo::storage::packing::calculate_packed_size;

#[derive(Copy, Drop, Serde, Debug, PartialEq)]
pub struct FieldLayout {
    pub selector: felt252,
    pub layout: Layout,
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

#[generate_trait]
pub impl LayoutImpl of LayoutTrait {
    fn is_same_type_of(self: @Layout, old: @Layout) -> bool {
        match (self, old) {
            (Layout::Fixed(_), Layout::Fixed(_)) => true,
            (Layout::Struct(_), Layout::Struct(_)) => true,
            (Layout::Tuple(_), Layout::Tuple(_)) => true,
            (Layout::Array(_), Layout::Array(_)) => true,
            (Layout::ByteArray, Layout::ByteArray) => true,
            (Layout::Enum(_), Layout::Enum(_)) => true,
            _ => false,
        }
    }
    fn struct_fields(self: @Layout) -> Span<FieldLayout> {
        match self {
            Layout::Struct(fields) => *fields,
            _ => { panic!("Unexpected layout type for a Struct.") },
        }
    }
}

#[generate_trait]
pub impl FieldLayoutsImpl of FieldLayoutsTrait {
    fn selectors(self: Span<FieldLayout>) -> Array<felt252> {
        let mut selectors: Array<felt252> = Default::default();
        for field_layout in self {
            selectors.append(*field_layout.selector);
        };
        selectors
    }
}

/// Compute the full size in bytes of a layout, when all the fields
/// are bit-packed.
/// Could be None if at least a field has a dynamic size.
pub fn compute_packed_size(layout: Layout) -> Option<usize> {
    if let Layout::Fixed(layout) = layout {
        let mut span_layout = layout;
        Option::Some(calculate_packed_size(ref span_layout))
    } else {
        Option::None
    }
}

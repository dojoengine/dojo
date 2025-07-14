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
    FixedArray: Span<(Layout, u32)>,
    ByteArray,
    // there is one layout per variant.
    // the `selector` field identifies the variant
    // the `layout` defines the variant data (could be empty for variant without data).
    Enum: Span<FieldLayout>,
}

#[generate_trait]
pub impl LayoutCompareImpl of LayoutCompareTrait {
    fn is_same_type_of(self: @Layout, old: @Layout) -> bool {
        match (self, old) {
            (Layout::Fixed(_), Layout::Fixed(_)) => true,
            (Layout::Struct(_), Layout::Struct(_)) => true,
            (Layout::Tuple(_), Layout::Tuple(_)) => true,
            (Layout::Array(_), Layout::Array(_)) => true,
            (Layout::FixedArray(_), Layout::FixedArray(_)) => true,
            (Layout::ByteArray, Layout::ByteArray) => true,
            (Layout::Enum(_), Layout::Enum(_)) => true,
            _ => false,
        }
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

/// With the new Dojo storage management (DojoStore trait),
/// variants start from 1, while they were started from 0 in
/// the legacy Dojo storage system.
/// To still support legacy Dojo models, we have to rebuild the
/// legacy storage layout from the new storage layout, meaning that
/// variant selectors have to be decremented by one.
pub fn build_legacy_layout(layout: Layout) -> Layout {
    match layout {
        Layout::Enum(field_layouts) => {
            let mut new_field_layouts = array![];

            for field_layout in field_layouts {
                new_field_layouts
                    .append(
                        FieldLayout {
                            selector: *field_layout.selector - 1,
                            layout: build_legacy_layout(*field_layout.layout),
                        },
                    );
            }

            Layout::Enum(new_field_layouts.span())
        },
        Layout::Struct(field_layouts) => {
            let mut new_field_layouts = array![];

            for field_layout in field_layouts {
                new_field_layouts
                    .append(
                        FieldLayout {
                            selector: *field_layout.selector,
                            layout: build_legacy_layout(*field_layout.layout),
                        },
                    );
            }

            Layout::Struct(new_field_layouts.span())
        },
        Layout::Tuple(item_layouts) => {
            let mut new_item_layouts = array![];

            for item_layout in item_layouts {
                new_item_layouts.append(build_legacy_layout(*item_layout));
            }

            Layout::Tuple(new_item_layouts.span())
        },
        Layout::Array(item_layouts) => {
            let mut new_item_layouts = array![];

            for item_layout in item_layouts {
                new_item_layouts.append(build_legacy_layout(*item_layout));
            }

            Layout::Array(new_item_layouts.span())
        },
        _ => layout,
    }
}

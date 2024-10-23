use dojo::meta::{Layout, FieldLayout};

/// find a field with its selector in a list of layouts
pub fn find_field_layout(
    field_selector: felt252, field_layouts: Span<FieldLayout>
) -> Option<Layout> {
    let mut i = 0;
    let layout = loop {
        if i >= field_layouts.len() {
            break Option::None;
        }

        let field_layout = *field_layouts.at(i);
        if field_selector == field_layout.selector {
            break Option::Some(field_layout.layout);
        }

        i += 1;
    };

    layout
}

/// Find the layout of a model field based on its selector.
///
/// # Arguments
///
/// * `model_layout` - The full model layout (must be a Layout::Struct).
/// *  `member_selector` - The model field selector.
///
/// # Returns
/// Some(Layout) if the field layout has been found, None otherwise.
pub fn find_model_field_layout(model_layout: Layout, member_selector: felt252) -> Option<Layout> {
    match model_layout {
        Layout::Struct(field_layouts) => { find_field_layout(member_selector, field_layouts) },
        _ => {
            // should never happen as model layouts are always struct layouts.
            core::panic_with_felt252('Unexpected model layout');
            Option::None
        }
    }
}

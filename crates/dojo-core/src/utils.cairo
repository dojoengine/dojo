/// Compute the poseidon hash of a serialized ByteArray
fn hash(data: @ByteArray) -> felt252 {
    let mut serialized = ArrayTrait::new();
    Serde::serialize(data, ref serialized);
    poseidon::poseidon_hash_span(serialized.span())
}

/// Computes the entity id from the keys.
///
/// # Arguments
///
/// * `keys` - The keys of the entity.
///
/// # Returns
///
/// The entity id.
fn entity_id_from_keys(keys: Span<felt252>) -> felt252 {
    poseidon::poseidon_hash_span(keys)
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
fn find_model_field_layout(
    model_layout: dojo::database::introspect::Layout, member_selector: felt252
) -> Option<dojo::database::introspect::Layout> {
    match model_layout {
        dojo::database::introspect::Layout::Struct(struct_layout) => {
            let mut i = 0;
            let layout = loop {
                if i >= struct_layout.len() {
                    break Option::None;
                }

                let field_layout = *struct_layout.at(i);
                if field_layout.selector == member_selector {
                    break Option::Some(field_layout.layout);
                }
                i += 1;
            };

            layout
        },
        _ => {
            // should never happen as model layouts are always struct layouts.
            panic_with_felt252('Unexpected model layout');
            Option::None
        }
    }
}

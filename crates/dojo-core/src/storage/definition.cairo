use super::database;

const DOJO_MODEL_LAYOUT_TABLE: felt252 = 'DOJO_MODEL_LAYOUT';
const DOJO_MODEL_TY_TABLE: felt252 = 'DOJO_MODEL_TY';

fn write_model_definition_item(table: felt252, model_selector: felt252, item: Span<felt252>) {
    let item_size = item.len();

    // first, write the item size
    database::set(table, model_selector, [item_size.into()].span(), 0, [32].span());

    // then, write the item data in a dedicated slot (table + 1) to not override its size.
    database::set_array(table + 1, model_selector, item, 0, item_size);
}

fn read_model_definition_item(table: felt252, model_selector: felt252) -> Span<felt252> {
    let res = database::get(table, model_selector, [32].span());
    assert(res.len() == 1, 'internal database error');

    let array_len = *res.at(0);
    assert(array_len.into() <= database::MAX_ARRAY_LENGTH, 'invalid array length');

    database::get_array(table + 1, model_selector, array_len.try_into().unwrap())
}

pub fn write_model_layout(model_selector: felt252, layout: Span<felt252>) {
    write_model_definition_item(DOJO_MODEL_LAYOUT_TABLE, model_selector, layout);
}

pub fn read_model_layout(model_selector: felt252) -> Span<felt252> {
    read_model_definition_item(DOJO_MODEL_LAYOUT_TABLE, model_selector)
}

pub fn write_model_ty(model_selector: felt252, ty: Span<felt252>) {
    write_model_definition_item(DOJO_MODEL_TY_TABLE, model_selector, ty);
}

pub fn read_model_ty(model_selector: felt252) -> Span<felt252> {
    read_model_definition_item(DOJO_MODEL_TY_TABLE, model_selector)
}

use array::ArrayTrait;
use array::SpanTrait;
use traits::Into;

use dojo_core::database::storage;

#[test]
#[available_gas(2000000)]
fn test_storage() {
    let mut keys = ArrayTrait::new();
    keys.append(0x1337);

    let mut values = ArrayTrait::new();
    values.append(0x1);
    values.append(0x2);

    storage::set(0, keys.span(), *values.at(0));
    assert(storage::get(0, keys.span()) == *values.at(0), 'value not set');

    storage::set_many(0, keys.span(), 0, values.span());
    let res = storage::get_many(0, keys.span(), 0, 2);
    assert(*res.at(0) == *values.at(0), 'value not set');
}

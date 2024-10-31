use core::array::ArrayTrait;
use core::array::SpanTrait;

use dojo::storage::storage;

#[test]
#[available_gas(2000000)]
fn test_storage() {
    let mut keys = ArrayTrait::new();
    keys.append(0x1337);

    let mut values = ArrayTrait::new();
    values.append(0x1);
    values.append(0x2);

    let layout = [251, 251].span();

    storage::set(0, keys.span(), *values.at(0));
    assert(storage::get(0, keys.span()) == *values.at(0), 'value not set');

    storage::set_many(0, keys.span(), values.span(), 0, layout).unwrap();
    let res = storage::get_many(0, keys.span(), layout).unwrap();
    assert(*res.at(0) == *values.at(0), 'value not set');
    assert(*res.at(1) == *values.at(1), 'value not set');
}

#[test]
#[available_gas(20000000)]
fn test_storage_empty() {
    let mut keys = ArrayTrait::new();
    assert(storage::get(0, keys.span()) == 0x0, 'Value should be 0');
    let many = storage::get_many(0, keys.span(), [251, 251, 251].span()).unwrap();
    assert(many.len() == 0x3, 'Array should be len 3');
    assert((*many[0]) == 0x0, 'Array[0] should be 0');
    assert((*many[1]) == 0x0, 'Array[1] should be 0');
    assert((*many[2]) == 0x0, 'Array[2] should be 0');

    let many = storage::get_many(0, keys.span(), [8, 8, 32].span()).unwrap();
    assert(many.len() == 0x3, 'Array should be len 3');
    assert((*many[0]) == 0x0, 'Array[0] should be 0');
    assert((*many[1]) == 0x0, 'Array[1] should be 0');
    assert((*many[2]) == 0x0, 'Array[2] should be 0');
}

#[test]
#[available_gas(2000000)]
fn test_storage_set_many() {
    let mut keys = ArrayTrait::new();
    keys.append(0x966);

    let mut values = ArrayTrait::new();
    values.append(0x1);
    values.append(0x2);
    values.append(0x3);
    values.append(0x4);

    storage::set_many(0, keys.span(), values.span(), 0, [251, 251, 251, 251].span()).unwrap();
    let many = storage::get_many(0, keys.span(), [251, 251, 251, 251].span()).unwrap();
    assert(many.at(0) == values.at(0), 'Value at 0 not equal!');
    assert(many.at(1) == values.at(1), 'Value at 1 not equal!');
    assert(many.at(2) == values.at(2), 'Value at 2 not equal!');
    assert(many.at(3) == values.at(3), 'Value at 3 not equal!');
}

#[test]
#[available_gas(2000000000)]
fn test_storage_set_many_several_segments() {
    let mut keys = ArrayTrait::new();
    keys.append(0x966);

    let mut layout = ArrayTrait::new();
    let mut values = ArrayTrait::new();
    let mut i = 0;
    loop {
        if i == 1000 {
            break;
        }

        values.append(i);
        layout.append(251_u8);

        i += 1;
    };

    storage::set_many(0, keys.span(), values.span(), 0, layout.span()).unwrap();
    let many = storage::get_many(0, keys.span(), layout.span()).unwrap();

    let mut i = 0;
    loop {
        if i == 1000 {
            break;
        }

        assert(many.at(i) == values.at(i), 'Value not equal!');

        i += 1;
    };
}

use array::ArrayTrait;
use array::SpanTrait;
use traits::Into;
use debug::PrintTrait;

use dojo::database::storage;

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

#[test]
#[available_gas(2000000)]
fn test_storage_empty() {
    let mut keys = ArrayTrait::new();
    assert(storage::get(0, keys.span()) == 0x0, 'Value should be 0');
    let many = storage::get_many(0, keys.span(), 0, 3);
    assert(*many.at(0) == 0x0, 'Value should be 0');
    assert(*many.at(1) == 0x0, 'Value should be 0');
    assert(*many.at(2) == 0x0, 'Value should be 0');
}

#[test]
#[available_gas(100000000)]
fn test_storage_get_many_length() {
    let mut keys = ArrayTrait::new();
    let mut i = 0_usize;
    loop {
        if i >= 30 {
            break;
        };
        assert(storage::get_many(0, keys.span(), 0, i).len() == i, 'Values should be equal!');
        i += 1;
    };
    
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

    storage::set_many(0, keys.span(), 0, values.span());
    let many = storage::get_many(0, keys.span(), 0, 4);
    assert(many.at(0) == values.at(0), 'Value at 0 not equal!');
    assert(many.at(1) == values.at(1), 'Value at 1 not equal!');
    assert(many.at(2) == values.at(2), 'Value at 2 not equal!');
    assert(many.at(3) == values.at(3), 'Value at 3 not equal!');
}

#[test]
#[available_gas(20000000)]
fn test_storage_set_many_with_offset() {
    let mut keys = ArrayTrait::new();
    keys.append(0x1364);

    let mut values = ArrayTrait::new();
    values.append(0x1);
    values.append(0x2);
    values.append(0x3);
    values.append(0x4);

    storage::set_many(0, keys.span(), 1, values.span());
    let many = storage::get_many(0, keys.span(), 0, 5);
    assert(*many.at(0) == 0x0, 'Value at 0 not equal!');
    assert(many.at(1) == values.at(0), 'Value at 1 not equal!');
    assert(many.at(2) == values.at(1), 'Value at 2 not equal!');
    assert(many.at(3) == values.at(2), 'Value at 3 not equal!');
    assert(many.at(4) == values.at(3), 'Value at 4 not equal!');
}


use array::ArrayTrait;
use traits::Into;
use debug::PrintTrait;
use option::OptionTrait;


use dojo::database::index;

#[test]
#[available_gas(2000000)]
fn test_index_same_values() {
    let no_get = index::get(0, 69, 0);
    assert(no_get.len() == 0, 'entity indexed');

    index::create(0, 69, 420, 0);
    let get = index::get(0, 69, 0);
    assert(get.len() == 1, 'entity not indexed');
    assert(*get.at(0) == 420, 'entity value incorrect');

    index::create(0, 69, 420, 0);
    let noop_get = index::get(0, 69, 0);
    assert(noop_get.len() == 1, 'index should be noop');

    index::create(0, 69, 1337, 0);
    let two_get = index::get(0, 69, 0);
    assert(two_get.len() == 2, 'index should have two get');
    assert(*two_get.at(1) == 1337, 'entity value incorrect');
}

#[test]
#[available_gas(2000000)]
fn test_index_different_values() {
    index::create(0, 69, 420, 1);
    let get = index::get(0, 69, 1);
    assert(get.len() == 1, 'entity not indexed');
    assert(*get.at(0) == 420, 'entity value incorrect');

    let noop_get = index::get(0, 69, 3);
    assert(noop_get.len() == 0, 'index should be noop');

    index::create(0, 69, 1337, 2);
    index::create(0, 69, 1337, 2);
    index::create(0, 69, 1338, 2);
    let two_get = index::get(0, 69, 2);
    assert(two_get.len() == 2, 'index should have two get');
    assert(*two_get.at(1) == 1338, 'two get value incorrect');
}

#[test]
#[available_gas(2000000)]
fn test_index_pagination() {
    index::create(0, 69, 1337, 0);
    index::create(0, 69, 1338, 0);
    let second = index::get_at(0, 69, 0, 1);
    let third = index::get_at(0, 69, 0, 2);
    assert(second == Option::Some(1338), 'second get value incorrect');
    assert(third == Option::None, 'third get value incorrect');
}

#[test]
#[available_gas(100000000)]
fn test_entity_delete_basic() {
    index::create(0, 69, 420, 1);
    let get = index::get(0, 69, 1);
    assert(get.len() == 1, 'entity not indexed');
    assert(*get.at(0) == 420, 'entity value incorrect');

    assert(index::exists(0, 69, 420), 'entity should exist');

    index::delete(0, 69, 420, array![].span());

    assert(!index::exists(0, 69, 420), 'entity should not exist');
    let no_get = index::get(0, 69, 1);
    assert(no_get.len() == 0, 'index should have no get');
}

#[test]
#[available_gas(100000000)]
fn test_entity_get_delete_shuffle() {
    let table = 1;
    index::create(0, table, 10, 1);
    index::create(0, table, 20, 1);
    index::create(0, table, 30, 1);
    assert(index::get(0, table, 1).len() == 3, 'wrong size');

    index::delete(0, table, 10);
    let entities = index::get(0, table, 1);
    assert(entities.len() == 2, 'wrong size');
    assert(*entities.at(0) == 30, 'idx 0 not 30');
    assert(*entities.at(1) == 20, 'idx 1 not 20');
}

#[test]
#[available_gas(100000000)]
fn test_entity_get_delete_non_existing() {
    assert(index::get(0, 69, 1).len() == 0, 'table len != 0');
    index::delete(0, 69, 999); // deleting non-existing should not panic
}

#[test]
#[available_gas(100000000)]
fn test_entity_delete_right_value() {
    let table = 1;
    index::create(0, table, 10, 1);
    index::create(0, table, 20, 2);
    index::create(0, table, 30, 2);
    assert(index::get(0, table, 2).len() == 2, 'wrong size');

    index::delete(0, table, 20);
    assert(index::exists(0, table, 20) == false, 'deleted value exists');
    let entities = index::get(0, table, 2);
    assert(entities.len() == 1, 'wrong size');
    assert(*entities.at(0) == 30, 'idx 0 not 30');
    
    assert(index::get(0, table, 1).len() == 1, 'wrong size');
}

#[test]
#[available_gas(20000000)]
fn test_with_keys_deletion() {
    let keys = array!['animal', 'barks'].span();
    let other_keys = array!['animal', 'meows'].span();
    let keys_layout = array![251, 251].span();

    index::create_with_keys(0, 69, 420, keys, keys_layout);
    index::create_with_keys(0, 69, 421, other_keys, keys_layout);

    let (ids, keys) = index::get_with_keys(0, 69, keys_layout);
    assert(ids.len() == 2, 'Not enough entities indexed');
    assert(keys.len() == 2, 'Lengths of keys inconsistent');
    assert(*ids.at(0) == 420, 'Identity value incorrect');
    assert(*ids.at(1) == 421, 'Identity value incorrect');

    assert(*(*keys.at(0)).at(1) == 'barks', 'Key at position 0 incorrect');
    assert(*(*keys.at(1)).at(1) == 'meows', 'Key at position 1 incorrect');

    // TODO: fix this
    // index::delete(0, 69, 420);

    // let (ids, keys) = index::get_with_keys(0, 69, keys.len());
    // assert(ids.len() == 1, 'Not enough entities indexed');
    // assert(keys.len() == 1, 'Lengths of keys inconsistent');
    // assert(*ids.at(0) == 421, 'Identity value incorrect');
    // assert(*(*keys.at(0)).at(1) == 'meows', 'Key at position 1 incorrect');
}

#[test]
#[available_gas(20000000)]
fn test_get_by_keys() {
    let keys = array!['animal', 'barks'].span();
    let other_keys = array!['animal', 'meows'].span();
    let keys_layout = array![251, 251].span();

    index::create_with_keys(0, 69, 420, keys, keys_layout);
    index::create_with_keys(0, 69, 421, other_keys, keys_layout);

    let ids = index::get_by_key(0, 69, 'animal');
    assert(ids.len() == 2, 'Incorrect number of entities');
    assert(*ids.at(0) == 420, 'Identity value incorrect at 0');
    assert(*ids.at(1) == 421, 'Identity value incorrect at 1');

    let ids = index::get_by_key(0, 69, 'barks');
    assert(ids.len() == 1, 'Incorrect number of entities');
    assert(*ids.at(0) == 420, 'Identity value incorrect at 0');
}

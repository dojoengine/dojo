use array::ArrayTrait;
use traits::Into;

use dojo_core::database::index;

#[test]
#[available_gas(2000000)]
fn test_index_entity() {
    let no_query = index::get(0, 69);
    assert(no_query.len() == 0, 'entity indexed');

    index::create(0, 69, 420);
    let query = index::get(0, 69);
    assert(query.len() == 1, 'entity not indexed');
    assert(*query.at(0) == 420, 'entity value incorrect');

    index::create(0, 69, 420);
    let noop_query = index::get(0, 69);
    assert(noop_query.len() == 1, 'index should be noop');

    index::create(0, 69, 1337);
    let two_query = index::get(0, 69);
    assert(two_query.len() == 2, 'index should have two query');
    assert(*two_query.at(1) == 1337, 'entity value incorrect');
}

#[test]
#[available_gas(2000000)]
fn test_entity_delete_basic() {
    index::create(0, 69, 420);
    let query = index::get(0, 69);
    assert(query.len() == 1, 'entity not indexed');
    assert(*query.at(0) == 420, 'entity value incorrect');

    assert(index::exists(0, 69, 420), 'entity should exist');

    index::delete(0, 69, 420);

    assert(!index::exists(0, 69, 420), 'entity should not exist');
    let no_query = index::get(0, 69);
    assert(no_query.len() == 0, 'index should have no query');
}

#[test]
#[available_gas(20000000)]
fn test_entity_query_delete_shuffle() {
    let table = 1;
    index::create(0, table, 10);
    index::create(0, table, 20);
    index::create(0, table, 30);
    assert(index::get(0, table).len() == 3, 'wrong size');

    index::delete(0, table, 10);
    let entities = index::get(0, table);
    assert(entities.len() == 2, 'wrong size');
    assert(*entities.at(0) == 30, 'idx 0 not 30');
    assert(*entities.at(1) == 20, 'idx 1 not 20');
}

#[test]
#[available_gas(20000000)]
fn test_entity_query_delete_non_existing() {
    assert(index::get(0, 69).len() == 0, 'table len != 0');
    index::delete(0, 69, 999); // deleting non-existing should not panic
}

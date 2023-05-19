use array::ArrayTrait;
use traits::Into;

use dojo_core::integer::u250;
use dojo_core::storage::index::Index;

#[test]
#[available_gas(2000000)]
fn test_index_entity() {
    let no_query = Index::query(69.into());
    assert(no_query.len() == 0, 'entity indexed');

    Index::create(69.into(), 420.into());
    let query = Index::query(69.into());
    assert(query.len() == 1, 'entity not indexed');
    assert(*query.at(0) == 420.into(), 'entity value incorrect');

    Index::create(69.into(), 420.into());
    let noop_query = Index::query(69.into());
    assert(noop_query.len() == 1, 'index should be noop');

    Index::create(69.into(), 1337.into());
    let two_query = Index::query(69.into());
    assert(two_query.len() == 2, 'index should have two query');
    assert(*two_query.at(1) == 1337.into(), 'entity value incorrect');
}

#[test]
#[available_gas(2000000)]
fn test_entity_delete_basic() {
    Index::create(69.into(), 420.into());
    let query = Index::query(69.into());
    assert(query.len() == 1, 'entity not indexed');
    assert(*query.at(0) == 420.into(), 'entity value incorrect');

    assert(Index::exists(69.into(), 420.into()), 'entity should exist');

    Index::delete(69.into(), 420.into());

    assert(!Index::exists(69.into(), 420.into()), 'entity should not exist');
    let no_query = Index::query(69.into());
    assert(no_query.len() == 0, 'index should have no query');
}

#[test]
#[available_gas(20000000)]
fn test_entity_query_delete_shuffle() {
    let table = 1.into();
    Index::create(table, 10.into());
    Index::create(table, 20.into());
    Index::create(table, 30.into());
    assert(Index::query(table).len() == 3, 'wrong size');

    Index::delete(table, 10.into());
    let entities = Index::query(table);
    assert(entities.len() == 2, 'wrong size');
    assert(*entities.at(0) == 30.into(), 'idx 0 not 30');
    assert(*entities.at(1) == 20.into(), 'idx 1 not 20');
}

#[test]
#[available_gas(20000000)]
fn test_entity_query_delete_non_existing() {
    assert(Index::query(69.into()).len() == 0, 'table len != 0');
    Index::delete(69.into(), 999.into()); // deleting non-existing should not panic
}

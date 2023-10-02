use array::ArrayTrait;
use traits::Into;
use debug::PrintTrait;
use option::OptionTrait;


use dojo::database::index;

#[test]
#[available_gas(2000000)]
fn test_index_entity() {
    let no_query = index::query(0, 69, Option::None(()));
    assert(no_query.len() == 0, 'entity indexed');

    index::create(0, 69, 420);
    let query = index::query(0, 69, Option::None(()));
    assert(query.len() == 1, 'entity not indexed');
    assert(*query.at(0) == 420, 'entity value incorrect');

    index::create(0, 69, 420);
    let noop_query = index::query(0, 69, Option::None(()));
    assert(noop_query.len() == 1, 'index should be noop');

    index::create(0, 69, 1337);
    let two_query = index::query(0, 69, Option::None(()));
    assert(two_query.len() == 2, 'index should have two query');
    assert(*two_query.at(1) == 1337, 'entity value incorrect');
}

#[test]
#[available_gas(2000000)]
fn test_entity_delete_basic() {
    index::create(0, 69, 420);
    let query = index::query(0, 69, Option::None(()));
    assert(query.len() == 1, 'entity not indexed');
    assert(*query.at(0) == 420, 'entity value incorrect');

    assert(index::exists(0, 69, 420), 'entity should exist');

    index::delete(0, 69, 420, array![].span());

    assert(!index::exists(0, 69, 420), 'entity should not exist');
    let no_query = index::query(0, 69, Option::None(()));
    assert(no_query.len() == 0, 'index should have no query');
}

#[test]
#[available_gas(20000000)]
fn test_entity_query_delete_shuffle() {
    let table = 1;
    index::create(0, table, 10);
    index::create(0, table, 20);
    index::create(0, table, 30);
    assert(index::query(0, table, Option::None(())).len() == 3, 'wrong size');

    index::delete(0, table, 10, array![].span());
    let entities = index::query(0, table, Option::None(()));
    assert(entities.len() == 2, 'wrong size');
    assert(*entities.at(0) == 30, 'idx 0 not 30');
    assert(*entities.at(1) == 20, 'idx 1 not 20');
}

#[test]
#[available_gas(20000000)]
fn test_entity_query_delete_non_existing() {
    assert(index::query(0, 69, Option::None(())).len() == 0, 'table len != 0');
    index::delete(0, 69, 999, array![].span()); // deleting non-existing should not panic
}

// #[test]
// #[available_gas(20000000)]
// fn test_with_keys() {
//     let keys = array!['animal', 'barks', 'brown'].span();
//     let keys_layout = array![251, 251, 251].span();

//     index::create_with_keys(0, 69, 420, keys, keys_layout);
//     let (ids, keys) = index::query_with_keys(0, 69, keys_layout);
//     assert(ids.len() == 1, 'entity not indexed');
//     assert(keys.len() == 1, 'entity not indexed');
//     assert(*ids.at(0) == 420, 'entity value incorrect');

//     assert(*(*keys.at(0)).at(0) == 'animal', 'key incorrect at 0');
//     assert(*(*keys.at(0)).at(1) == 'barks', 'key incorrect at 1');
//     assert(*(*keys.at(0)).at(2) == 'brown', 'key incorrect at 2');
// }

// #[test]
// #[available_gas(20000000)]
// fn test_with_keys_deletion() {
//     let keys = array!['animal', 'barks'].span();
//     let other_keys = array!['animal', 'meows'].span();
//     let keys_layout = array![251, 251].span();

//     index::create_with_keys(0, 69, 420, keys, keys_layout);
//     index::create_with_keys(0, 69, 421, other_keys, keys_layout);

//     let (ids, keys) = index::query_with_keys(0, 69, keys_layout);
//     assert(ids.len() == 2, 'Not enough entities indexed');
//     assert(keys.len() == 2, 'Lengths of keys inconsistent');
//     assert(*ids.at(0) == 420, 'Identity value incorrect');
//     assert(*ids.at(1) == 421, 'Identity value incorrect');

//     assert(*(*keys.at(0)).at(1) == 'barks', 'Key at position 0 incorrect');
//     assert(*(*keys.at(1)).at(1) == 'meows', 'Key at position 1 incorrect');

    // TODO: fix this
    // index::delete(0, 69, 420);

    // let (ids, keys) = index::query_with_keys(0, 69, keys.len());
    // assert(ids.len() == 1, 'Not enough entities indexed');
    // assert(keys.len() == 1, 'Lengths of keys inconsistent');
    // assert(*ids.at(0) == 421, 'Identity value incorrect');
    // assert(*(*keys.at(0)).at(1) == 'meows', 'Key at position 1 incorrect');
// }

// #[test]
// #[available_gas(20000000)]
// fn test_get_by_keys() {
//     let keys = array!['animal', 'barks'].span();
//     let other_keys = array!['animal', 'meows'].span();
//     let keys_layout = array![251, 251].span();

//     index::create_with_keys(0, 69, 420, keys, keys_layout);
//     index::create_with_keys(0, 69, 421, other_keys, keys_layout);

//     let ids = index::query_by_key(0, 69, 'animal');
//     assert(ids.len() == 2, 'Incorrect number of entities');
//     assert(*ids.at(0) == 420, 'Identity value incorrect at 0');
//     assert(*ids.at(1) == 421, 'Identity value incorrect at 1');

//     let ids = index::query_by_key(0, 69, 'barks');
//     assert(ids.len() == 1, 'Incorrect number of entities');
//     assert(*ids.at(0) == 420, 'Identity value incorrect at 0');
// }

// #[test]
// #[available_gas(20000000)]
// fn test_create_new_entry() {
//     let address_domain = 0;
//     let index = 100;
//     let id = 200;
    
//     index::create(address_domain, index, id);

//     assert(index::exists(address_domain, index, id), 'Function saves data incorrectly');
// }

// #[test]
// #[available_gas(20000000)]
// fn test_add_key() {
//     let address_domain = 0;
//     let index = 420;
//     let id = 123;
//     let idx = 7;

//     let mut key = ArrayTrait::new();
//     key.append(69);
//     key.append(111);
//     key.append(420);

//     let x = index::add_key(address_domain, index, id, *key.at(0), idx);
//     assert(x == 0, 'Expected value incorrect at 0');

//     let x = index::add_key(address_domain, index, id, *key.at(1), idx);
//     assert(x == 0, 'Expected value incorrect at 1');

//     let x = index::add_key(address_domain, index, id, *key.at(2), idx);
//     assert(x == 0, 'Expected value incorrect at 2');
    
//     let x = index::add_key(address_domain, index, id, *key.at(0), idx);
//     assert(x == 1, 'Expected value incorrect at 0');

//     let x = index::add_key(address_domain, index, id, *key.at(0), idx);
//     assert(x == 2, 'Expected value incorrect at 0');
// }

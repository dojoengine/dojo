use array::{ArrayTrait, SpanTrait};
use traits::{Into, TryInto};
use serde::Serde;
use hash::LegacyHash;
use poseidon::poseidon_hash_span;

mod index;
#[cfg(test)]
mod index_test;
mod introspect;
#[cfg(test)]
mod introspect_test;
mod storage;
#[cfg(test)]
mod storage_test;
mod utils;
#[cfg(test)]
mod utils_test;

use index::WhereCondition;

#[derive(Copy, Drop, Serde)]
struct KeyValuesClause {
    key: felt252,
    values: Span<felt252>,
}

// Could be replaced with a `KeyValues` with one value, 
// but this allows us to avoid hashing, and is most common.
#[derive(Copy, Drop, Serde)]
struct KeyValueClause {
    key: felt252,
    value: felt252,
}

#[derive(Copy, Drop, Serde)]
enum QueryClause {
    KeyValue: KeyValueClause,
    KeyValues: KeyValuesClause,
    All: (),
}

fn get(
    table: felt252, key: felt252, offset: u8, length: usize, layout: Span<u8>
) -> Span<felt252> {
    let mut keys = ArrayTrait::new();
    keys.append('dojo_storage');
    keys.append(table);
    keys.append(key);
    storage::get_many(0, keys.span(), offset, length, layout)
}

fn set(table: felt252, key: felt252, offset: u8, value: Span<felt252>, layout: Span<u8>) {
    let mut keys = ArrayTrait::new();
    keys.append('dojo_storage');
    keys.append(table);
    keys.append(key);
    storage::set_many(0, keys.span(), offset, value, layout);
}

/// Creates an entry in the database and adds it to appropriate indexes.
/// # Arguments
/// * `table` - The table to create the entry in.
/// * `id` - id of the created entry.
/// * `keys` - The keys to index the entry by.
/// * `offset` - The offset of the entry.
/// * `value` - The value of the entry.
/// * `layout` - The layout of the entry.
fn set_with_index( 
    table: felt252,
    id: felt252,
    keys: Span<felt252>,
    offset: u8,
    value: Span<felt252>,
    layout: Span<u8>
) {
    set(table, id, offset, value, layout);
    index::create(0, table, id, 0); // create a record in index of all records

    let mut idx = 0;
    loop {
        if idx == keys.len() {
            break;
        }
        let index = poseidon_hash_span(array![table, idx.into()].span());

        index::create(0, index, id, *keys.at(idx)); // create a record for each of the keys
        
        idx += 1;
    };

    let len_keys = array!['dojo_storage_keys_len', table, id].span();
    storage::set(0, len_keys, keys.len().into()); // save the number of keys
}

fn del(table: felt252, key: felt252) {
    index::delete(0, table, key);

    let len_keys = array!['dojo_storage_keys_len', table, key].span();
    let len = storage::get(0, len_keys);

    let mut idx = 0;
    loop {
        if idx == len {
            break;
        }
        let index = poseidon_hash_span(array![table, idx].span());

        index::delete(0, index, key);
        
        idx += 1;
    };

    storage::set(0, len_keys, 0); // overwrite the number of keys
}

// Query all entities that meet a criteria. If no index is defined,
// Returns a tuple of spans, first contains the entity IDs,
// second the deserialized entities themselves.
fn scan(
    model: felt252, index: Option<felt252>, where: QueryClause, values_length: usize, values_layout: Span<u8>
) -> (Span<felt252>, Span<Span<felt252>>) {
    let all_ids = scan_ids(model, index, where);
    (all_ids, get_by_ids(model, all_ids, values_length, values_layout))
}

/// Analogous to `scan`, but returns only the IDs of the entities.
fn scan_ids(model: felt252, where: Option<WhereCondition>) -> Span<felt252> {
    match where {
        QueryClause::KeyValue(clause) => {
            let table = poseidon_hash_span(array![model, clause.key].span());

            match index {
                Option::Some(index) => match index::get_at(0, table, clause.value, index) {
                    Option::Some(id) => array![id],
                    Option::None => array![],
                }.span(),
                Option::None => index::get(0, table, clause.value)
            }
        },

        QueryClause::KeyValues(clause) => {
            let table = poseidon_hash_span(array![model, clause.key].span());
            let value = poseidon_hash_span(clause.values);

            match index {
                Option::Some(index) => match index::get_at(0, table, value, index) {
                    Option::Some(id) => array![id],
                    Option::None => array![],
                }.span(),
                Option::None => index::get(0, table, value)
            }
        },

        QueryClause::All => {
            index::get(0, model, 0)
        },
    }
}

/// Returns entries on the given ids.
/// # Arguments
/// * `table` - The table to get the entries from.
/// * `all_ids` - The ids of the entries to get.
/// * `length` - The length of the entries.
fn get_by_ids(
    table: felt252, all_ids: Span<felt252>, length: u32, layout: Span<u8>
) -> Span<Span<felt252>> {
    let mut entities: Array<Span<felt252>> = ArrayTrait::new();
    let mut ids = all_ids;
    loop {
        match ids.pop_front() {
            Option::Some(id) => {
                let mut keys = ArrayTrait::new();
                keys.append('dojo_storage');
                keys.append(table);
                keys.append(*id);
                let value: Span<felt252> = storage::get_many(0, keys.span(), 0_u8, length, layout);
                entities.append(value);
            },
            Option::None(_) => {
                break entities.span();
            }
        };
    }
}

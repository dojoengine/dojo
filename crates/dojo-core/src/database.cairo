use array::{ArrayTrait, SpanTrait};
use traits::{Into, TryInto};
use serde::Serde;
use hash::LegacyHash;
use poseidon::poseidon_hash_span;

mod index;
#[cfg(test)]
mod index_test;
mod schema;
mod storage;
#[cfg(test)]
mod storage_test;
mod utils;
#[cfg(test)]
mod utils_test;

use index::WhereCondition;

fn get(table: felt252, key: felt252, offset: u8, length: usize, layout: Span<u8>) -> Span<felt252> {
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

fn set_with_index( 
    table: felt252,
    key: felt252,
    keys: Span<felt252>,
    offset: u8,
    value: Span<felt252>,
    layout: Span<u8>
) {
    set(table, key, offset, value, layout);
    index::create(0, table, key, 0); // create a record in index of all records

    let mut idx = 0;
    loop {
        if idx == keys.len() {
            break;
        }
        let mut serialized = ArrayTrait::new();
        table.serialize(ref serialized);
        idx.serialize(ref serialized);
        let index = poseidon_hash_span(serialized.span());

        index::create(0, index, key, *keys.at(0)); // create a record for each of the keys
        
        idx += 1;
    };
}

fn del(table: felt252, key: felt252) {
    index::delete(0, table, key);
}

// Query all entities that meet a criteria. If no index is defined,
// Returns a tuple of spans, first contains the entity IDs,
// second the deserialized entities themselves.
fn scan(
    model: felt252, where: Option<WhereCondition>, values_length: usize, values_layout: Span<u8>
) -> (Span<felt252>, Span<Span<felt252>>) {
    let all_ids = scan_ids(model, where);
    (all_ids, get_by_ids(model, all_ids, values_length, values_layout))
}

/// Analogous to `scan`, but returns only the IDs of the entities.
fn scan_ids(model: felt252, where: Option<WhereCondition>) -> Span<felt252> {
    match where {
        Option::Some(clause) => {
            let mut serialized = ArrayTrait::new();
            model.serialize(ref serialized);
            clause.key.serialize(ref serialized);
            let index = poseidon_hash_span(serialized.span());

            let all_ids = index::get(0, index, clause.value);
            (all_ids, get_by_ids(model, all_ids, values_length, values_layout))
        },
        // If no `where` clause is defined, we return all values.
        Option::None(_) => {
            index::query(0, model, Option::None)
        }
    }
}

/// Returns entries on the given ids.
/// # Arguments
/// * `class_hash` - The class hash of the contract.
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

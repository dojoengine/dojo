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

// Could be replaced with a `KeyValues` with one value, 
// but this allows us to avoid hashing, and is most common.
#[derive(Copy, Drop, Serde)]
struct MemberClause {
    model: felt252,
    member: felt252,
    value: felt252,
}

#[derive(Copy, Drop, Serde)]
struct CompositeClause {
    operator: LogicalOperator,
    clauses: Span<Clause>,
}

#[derive(Copy, Drop, Serde)]
enum LogicalOperator {
    And,
}

#[derive(Copy, Drop, Serde)]
enum Clause {
    Member: MemberClause,
    Composite: CompositeClause,
    All: felt252,
}

fn get(model: felt252, key: felt252, offset: u8, length: usize, layout: Span<u8>) -> Span<felt252> {
    let mut keys = ArrayTrait::new();
    keys.append('dojo_storage');
    keys.append(model);
    keys.append(key);
    storage::get_many(0, keys.span(), offset, length, layout)
}

fn set(model: felt252, key: felt252, offset: u8, value: Span<felt252>, layout: Span<u8>) {
    let mut keys = ArrayTrait::new();
    keys.append('dojo_storage');
    keys.append(model);
    keys.append(key);
    storage::set_many(0, keys.span(), offset, value, layout);
}

/// Creates an entry in the database and adds it to appropriate indexes.
/// # Arguments
/// * `model` - The model to create the entry in.
/// * `key` - key of the created entry.
/// * `members` - The members to create an index on.
/// * `offset` - The offset of the entry.
/// * `value` - The value of the entry.
/// * `layout` - The layout of the entry.
fn set_with_index(
    model: felt252,
    key: felt252,
    members: Span<felt252>,
    offset: u8,
    values: Span<felt252>,
    layout: Span<u8>
) {
    set(model, key, offset, values, layout);
    index::create(0, model, key, 0); // create a record in index of all records

    let mut idx = 0;
    loop {
        if idx == members.len() {
            break;
        }

        let index = poseidon_hash_span(array![model, *members.at(idx)].span());
        index::create(0, index, key, *values.at(idx)); // create a record for each of the indexes
        idx += 1;
    };
}

fn del(model: felt252, key: felt252) {
    index::delete(0, model, key);

    let len_keys = array!['dojo_storage_keys_len', model, key].span();
    let len = storage::get(0, len_keys);

    let mut idx = 0;
    loop {
        if idx == len {
            break;
        }
        let index = poseidon_hash_span(array![model, idx].span());

        index::delete(0, index, key);

        idx += 1;
    };

    storage::set(0, len_keys, 0); // overwrite the number of keys
}

// Query all entities that meet a criteria. If no index is defined,
// Returns a tuple of spans, first contains the entity IDs,
// second the deserialized entities themselves.
fn scan(where: Clause, values_length: usize, values_layout: Span<u8>) -> Span<Span<felt252>> {
    match where {
        Clause::Member(clause) => {
            let i = poseidon_hash_span(array![clause.model, clause.member].span());
            let keys = index::get(0, i, clause.value);
            get_by_keys(clause.model, keys, values_length, values_layout)
        },
        Clause::Composite(clause) => {
            assert(false, 'unimplemented');
            array![array![].span()].span()
        },
        Clause::All(model) => {
            let keys = index::get(0, model, 0);
            get_by_keys(model, keys, values_length, values_layout)
        }
    }
}

/// Analogous to `scan`, but returns only the keys of the entities.
fn scan_keys(where: Clause) -> Span<felt252> {
    match where {
        Clause::Member(clause) => {
            let i = poseidon_hash_span(array![clause.model, clause.member].span());
            index::get(0, i, clause.value)
        },
        Clause::Composite(clause) => {
            assert(false, 'unimplemented');
            array![].span()
        },
        Clause::All(model) => {
            index::get(0, model, 0)
        }
    }
}

/// Returns entries on the given keys.
/// # Arguments
/// * `model` - The model to get the entries from.
/// * `keys` - The keys of the entries to get.
/// * `length` - The length of the entries.
fn get_by_keys(
    model: felt252, mut keys: Span<felt252>, length: u32, layout: Span<u8>
) -> Span<Span<felt252>> {
    let mut entities: Array<Span<felt252>> = ArrayTrait::new();

    loop {
        match keys.pop_front() {
            Option::Some(key) => {
                let keys = array!['dojo_storage', model, *key];
                let value: Span<felt252> = storage::get_many(0, keys.span(), 0_u8, length, layout);
                entities.append(value);
            },
            Option::None(_) => {
                break entities.span();
            }
        };
    }
}

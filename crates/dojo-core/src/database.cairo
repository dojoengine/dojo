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
struct MemberClause {
    model: felt252,
    member: felt252, // positon of the member in the model
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
            break; // Iterating over all members of the model with `#[key]` attribute
        }

        // The position of the member in the model identifies the index
        let index = poseidon_hash_span(array![model, idx.into()].span()); 
        index::create(0, index, key, *members.at(idx)); // create a record for each of the indexes
        idx += 1;
    };
}

fn del(model: felt252, key: felt252) {
    index::delete(0, model, key);

    let mut idx = 0; // Iterating over all members of the model...
    loop {
        let index = poseidon_hash_span(array![model, idx].span());

        if !index::exists(0, index, key) {
            break; // ...until we find a member without `#[key]` attribute
        }

        index::delete(0, index, key); // deleting all inbetween
        idx += 1;
    };
}

// Query all entities that meet a criteria. If no index is defined,
// Returns a tuple of spans, first contains the entity IDs,
// second the deserialized entities themselves.
fn scan(where: Clause, values_length: usize, values_layout: Span<u8>) -> Span<Span<felt252>> {
    match where {
        Clause::Member(clause) => {
            // The position of the member in the model identifies the index
            let index = poseidon_hash_span(array![clause.model, clause.member].span());
            let keys = index::get(0, index, clause.value);
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
            // The position of the member in the model identifies the index
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

fn get_by_key(
    class_hash: starknet::ClassHash, component: felt252, partition: felt252, key: felt252, length: usize
) -> (Span<felt252>, Span<Span<felt252>>) {
        let table = {
        if partition == 0.into() {
            component
        } else {
            let mut serialized = ArrayTrait::new();
            component.serialize(ref serialized);
            partition.serialize(ref serialized);
            let hash = poseidon_hash_span(serialized.span());
            hash.into()
        }
    };

    let all_ids = index::get_by_key(0, table, key);
    (all_ids.span(), get_by_ids(class_hash, table, all_ids.span(), length))
}

fn set_with_keys(
    class_hash: starknet::ClassHash, table: felt252, id: felt252, offset: u8, value: Span<felt252>, keys: Span<felt252>

) {
    set(class_hash, table, id, offset, value);
    index::create_with_keys(0, table, id, keys);
}
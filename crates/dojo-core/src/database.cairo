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

fn get(
    class_hash: starknet::ClassHash, table: felt252, key: felt252, offset: u8, length: usize, layout: Span<u8>
) -> Span<felt252> {
    let mut keys = ArrayTrait::new();
    keys.append('dojo_storage');
    keys.append(table);
    keys.append(key);
    storage::get_many(0, keys.span(), offset, length, layout)
}

fn set(
    class_hash: starknet::ClassHash, table: felt252, key: felt252, offset: u8, value: Span<felt252>, layout: Span<u8>
) {
    let mut keys = ArrayTrait::new();
    keys.append('dojo_storage');
    keys.append(table);
    keys.append(key);
    storage::set_many(0, keys.span(), offset, value, layout);
}

fn del(class_hash: starknet::ClassHash, table: felt252, key: felt252, keys_layout: Span<u8>) {
    // index::delete(0, table, key, keys_layout);
} 

// Query all entities that meet a criteria. If no index is defined,
// Returns a tuple of spans, first contains the entity IDs,
// second the deserialized entities themselves.
fn scan(
    class_hash: starknet::ClassHash, model: felt252, where: Option<WhereCondition>, values_length: usize, values_layout: Span<u8>
) -> (Span<felt252>, Span<Span<felt252>>) {
    match where {
        // 
        Option::Some(clause) => {
            let mut serialized = ArrayTrait::new();
            model.serialize(ref serialized);
            clause.key.serialize(ref serialized);
            let index = poseidon_hash_span(serialized.span());

            let all_ids = index::get_by_key(0, index, clause.value);
            (all_ids.span(), get_by_ids(class_hash, index, all_ids.span(), length, layout))
        },

        // If no `where` clause is defined, we return all values.
        Option::None(_) => {
            let all_ids = index::get(0, table);
            let mut ids = all_ids.span();
            (all_ids.span(), get_by_ids(class_hash, table, all_ids.span(), values_size, values_layout))
        }
    }
}

/// Returns entries on the given ids.
/// # Arguments
/// * `class_hash` - The class hash of the contract.
/// * `table` - The table to get the entries from.
/// * `all_ids` - The ids of the entries to get.
/// * `length` - The length of the entries.
fn get_by_ids(class_hash: starknet::ClassHash, table: felt252, all_ids: Span<felt252>, length: u32, layout: Span<u8>) -> Span<Span<felt252>> {
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

/// Set, but with writing keys to the appropriate indexes
/// # Arguments
/// * `class_hash` - The class hash of the contract.
/// * `table` - The table to set the entries to.
/// * `id` - The id of the entry to set.
/// * `offset` - The offset of the entry to set.
/// * `value` - The value of the entry to set.
/// * `keys` - The keys of the entry to set in the index.
fn set_with_keys(
    class_hash: starknet::ClassHash, table: felt252, id: felt252, offset: u8, value: Span<felt252>, layout: Span<u8>, keys: Span<felt252>, keys_layout: Span<u8>
) {
    set(class_hash, table, id, offset, value, layout);
    index::create_with_keys(0, table, id, keys, keys_layout);
}

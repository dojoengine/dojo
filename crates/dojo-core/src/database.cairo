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
    storage::set_many(0, keys.span(), offset, value);
    index::create(0, table, key);
}

fn del(class_hash: starknet::ClassHash, table: felt252, key: felt252) {
    index::delete(0, table, key);
}

// returns a tuple of spans, first contains the entity IDs,
// second the deserialized entities themselves
fn all(
    class_hash: starknet::ClassHash, model: felt252, partition: felt252, length: usize, layout: Span<u8>
) -> (Span<felt252>, Span<Span<felt252>>) {
    let table = {
        if partition == 0.into() {
            model
        } else {
            let mut serialized = ArrayTrait::new();
            model.serialize(ref serialized);
            partition.serialize(ref serialized);
            let hash = poseidon_hash_span(serialized.span());
            hash.into()
        }
    };

    let all_ids = index::get(0, table);
    let mut ids = all_ids.span();
  
    (all_ids.span(), get_by_ids(class_hash, table, all_ids.span(), length))
}

fn get_by_ids(class_hash: starknet::ClassHash, table: felt252, all_ids: Span<felt252>, length: u32) -> Span<Span<felt252>> {
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
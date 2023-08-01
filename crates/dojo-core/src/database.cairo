use array::{ArrayTrait, SpanTrait};
use traits::{Into, TryInto};
use serde::Serde;
use hash::LegacyHash;
use poseidon::poseidon_hash_span;

mod index;
mod storage;
mod utils;

fn get(
    class_hash: starknet::ClassHash, table: felt252, key: felt252, offset: u8, length: usize
) -> Span<felt252> {
    let mut keys = ArrayTrait::new();
    keys.append('dojo_storage');
    keys.append(table);
    keys.append(key);
    storage::get_many(0, keys.span(), offset, length)
}

fn set(
    class_hash: starknet::ClassHash, table: felt252, key: felt252, offset: u8, value: Span<felt252>
) {
    let mut keys = ArrayTrait::new();
    keys.append('dojo_storage');
    keys.append(table);
    keys.append(key);
    storage::set_many(0, keys.span(), offset, value);
}

fn del(class_hash: starknet::ClassHash, table: felt252, key: felt252) {
    index::delete(0, table, query.hash());
}

// returns a tuple of spans, first contains the entity IDs,
// second the deserialized entities themselves
fn all(
    class_hash: starknet::ClassHash, component: felt252, partition: felt252, length: usize
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

    let all_ids = index::get(0, table);
    let mut ids = all_ids.span();
    let mut entities: Array<Span<felt252>> = ArrayTrait::new();
    loop {
        match ids.pop_front() {
            Option::Some(id) => {
                let mut keys = ArrayTrait::new();
                keys.append('dojo_storage');
                keys.append(table);
                keys.append(*id);
                let value: Span<felt252> = storage::get_many(0, keys.span(), 0_u8, length);
                entities.append(value);
            },
            Option::None(_) => {
                break (all_ids.span(), entities.span());
            }
        };
    }
}

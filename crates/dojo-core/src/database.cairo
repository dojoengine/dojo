use array::{ArrayTrait, SpanTrait};
use traits::{Into, TryInto};
use serde::Serde;
use hash::LegacyHash;
use poseidon::poseidon_hash_span;

mod index;
mod query;
mod storage;
mod utils;

use dojo_core::database::{query::{Query, QueryTrait}};
use dojo_core::interfaces::{IComponentLibraryDispatcher, IComponentDispatcherTrait};

fn get(
    class_hash: starknet::ClassHash, table: felt252, query: Query, offset: u8, length: usize
) -> Option<Span<felt252>> {
    let mut length = length;
    if length == 0 {
        length = IComponentLibraryDispatcher { class_hash: class_hash }.len();
    }

    let id = query.hash();
    let mut keys = ArrayTrait::new();
    keys.append('dojo_storage');
    keys.append(table);
    keys.append(id);
    match index::exists(0, table, id) {
        bool::False(()) => Option::None(()),
        bool::True(()) => Option::Some(storage::get_many(0, keys.span(), offset, length)),
    }
}

fn set(
    class_hash: starknet::ClassHash, table: felt252, query: Query, offset: u8, value: Span<felt252>
) {
    let id = query.hash();

    let length = IComponentLibraryDispatcher { class_hash: class_hash }.len();
    assert(value.len() <= length, 'Value too long');

    index::create(0, table, id);

    let mut keys = ArrayTrait::new();
    keys.append('dojo_storage');
    keys.append(table);
    keys.append(id);
    storage::set_many(0, keys.span(), offset, value);
}

fn del(class_hash: starknet::ClassHash, table: felt252, query: Query) {
    index::delete(0, table, query.hash());
}

// returns a tuple of spans, first contains the entity IDs,
// second the deserialized entities themselves
fn all(
    class_hash: starknet::ClassHash, component: felt252, partition: felt252
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
    let length = IComponentLibraryDispatcher { class_hash: class_hash }.len();

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
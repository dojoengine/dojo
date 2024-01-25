use array::{ArrayTrait, SpanTrait};
use traits::{Into, TryInto};
use serde::Serde;
use hash::LegacyHash;
use poseidon::poseidon_hash_span;
use starknet::SyscallResultTrait;

const DOJO_STORAGE: felt252 = 'dojo_storage';

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

fn get(table: felt252, key: felt252, layout: Span<u8>) -> Span<felt252> {
    let mut keys = ArrayTrait::new();
    keys.append(DOJO_STORAGE);
    keys.append(table);
    keys.append(key);
    storage::get_many(0, keys.span(), layout).unwrap_syscall()
}

fn set(table: felt252, key: felt252, value: Span<felt252>, layout: Span<u8>) {
    let mut keys = ArrayTrait::new();
    keys.append(DOJO_STORAGE);
    keys.append(table);
    keys.append(key);
    storage::set_many(0, keys.span(), value, layout).unwrap_syscall();
}

fn set_with_index(
    table: felt252, key: felt252, value: Span<felt252>, layout: Span<u8>
) {
    set(table, key, value, layout);
    index::create(0, table, key);
}

fn del(table: felt252, key: felt252) {
    index::delete(0, table, key);
}

/// Query all entities that meet a criteria. If no index is defined,
/// Returns a tuple of spans, first contains the entity IDs,
/// second the deserialized entities themselves.
fn scan(
    model: felt252, values_layout: Span<u8>
) -> (Span<felt252>, Span<Span<felt252>>) {
    let all_ids = scan_ids(model);
    (all_ids, get_by_ids(model, all_ids, values_layout))
}

/// Analogous to `scan`, but returns only the IDs of the entities.
fn scan_ids(model: felt252) -> Span<felt252> {
    index::query(0, model)
}

/// Returns entries on the given ids.
///
/// # Arguments
///
/// * `table` - The table to get the entries from.
/// * `all_ids` - The ids of the entries to get.
/// * `layout` - The memory layout of the entity.
fn get_by_ids(
    table: felt252, all_ids: Span<felt252>, layout: Span<u8>
) -> Span<Span<felt252>> {
    let mut entities: Array<Span<felt252>> = ArrayTrait::new();
    let mut ids = all_ids;
    loop {
        match ids.pop_front() {
            Option::Some(id) => {
                let mut keys = ArrayTrait::new();
                keys.append(DOJO_STORAGE);
                keys.append(table);
                keys.append(*id);
                let value: Span<felt252> = storage::get_many(0, keys.span(), layout).unwrap_syscall();
                entities.append(value);
            },
            Option::None(_) => {
                break entities.span();
            }
        };
    }
}

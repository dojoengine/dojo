use array::{ArrayTrait, SpanTrait};
use traits::{Into, TryInto};
use serde::Serde;
use hash::LegacyHash;
use poseidon::poseidon_hash_span;
use starknet::SyscallResultTrait;

const DOJO_STORAGE: felt252 = 'dojo_storage';

mod introspect;
#[cfg(test)]
mod introspect_test;
mod storage;
#[cfg(test)]
mod storage_test;

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

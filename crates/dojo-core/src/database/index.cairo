use array::{ArrayTrait, SpanTrait};
use traits::Into;
use option::OptionTrait;
use poseidon::poseidon_hash_span;
use serde::Serde;

use dojo::database::storage;

fn create(address_domain: u32, index: felt252, id: felt252) {
    if exists(address_domain, index, id) {
        return ();
    }

    let index_len_key = build_index_len_key(index);
    let index_len = storage::get(address_domain, index_len_key);
    storage::set(address_domain, build_index_item_key(index, id), index_len + 1);
    storage::set(address_domain, index_len_key, index_len + 1);
    storage::set(address_domain, build_index_key(index, index_len), id);
}

fn create_with_keys(address_domain: u32, index: felt252, id: felt252, keys: Span<felt252>) {
    create(address_domain, index, id);
    storage::set_many(address_domain, build_index_item_keys(index, id), 0, keys);   // id -> keys
}

fn delete(address_domain: u32, index: felt252, id: felt252) {
    if !exists(address_domain, index, id) {
        return ();
    }

    let index_len_key = build_index_len_key(index);
    let replace_item_idx = storage::get(address_domain, index_len_key) - 1;

    let index_item_key = build_index_item_key(index, id);
    let delete_item_idx = storage::get(address_domain, index_item_key) - 1;

    storage::set(address_domain, index_item_key, 0);
    storage::set(address_domain, index_len_key, replace_item_idx);

    // Replace the deleted element with the last element.
    // NOTE: We leave the last element set as to not produce an unncessary state diff.
    let replace_item_value = storage::get(address_domain, build_index_key(index, replace_item_idx));
    storage::set(address_domain, build_index_key(index, delete_item_idx), replace_item_value);
}

fn exists(address_domain: u32, index: felt252, id: felt252) -> bool {
    storage::get(address_domain, build_index_item_key(index, id)) != 0
}

fn get(address_domain: u32, index: felt252) -> Array<felt252> {
    let mut res = ArrayTrait::new();

    let index_len_key = build_index_len_key(index);
    let index_len = storage::get(address_domain, index_len_key);
    let mut idx = 0;

    loop {
        if idx == index_len {
            break ();
        }

        res.append(storage::get(address_domain, build_index_key(index, idx)));
        idx += 1;
    };

    res
}

fn get_with_keys(address_domain: u32, index: felt252, key_length: usize) -> (Array<felt252>, Array<Span<felt252>>) {
    let mut ids = ArrayTrait::new();
    let mut all_keys = ArrayTrait::new();

    let index_len_key = build_index_len_key(index);
    let index_len = storage::get(address_domain, index_len_key);
    let mut idx = 0;

    loop {
        if idx == index_len {
            break ();
        }

        let id = storage::get(address_domain, build_index_key(index, idx));
        let keys = storage::get_many(address_domain, build_index_item_keys(index, id), 0, key_length);
        ids.append(id);
        all_keys.append(keys);
        idx += 1;
    };

    (ids, all_keys)
}

fn index_key_prefix() -> Array<felt252> {
    let mut prefix = ArrayTrait::new();
    prefix.append('dojo_index');
    prefix
}

fn build_index_len_key(index: felt252) -> Span<felt252> {
    let mut index_len_key = index_key_prefix();
    index_len_key.append('index_lens');
    index_len_key.append(index);
    index_len_key.span()
}

fn build_index_key(index: felt252, idx: felt252) -> Span<felt252> {
    let mut key = index_key_prefix();
    key.append('indexes');
    key.append(index);
    key.append(idx);
    key.span()
}

fn build_index_item_key(index: felt252, id: felt252) -> Span<felt252> {
    let mut index_len_key = index_key_prefix();
    index_len_key.append('index_ids');
    index_len_key.append(index);
    index_len_key.append(id);
    index_len_key.span()
}

fn build_index_item_keys(index: felt252, id: felt252) -> Span<felt252> {
    let mut index_len_key = index_key_prefix();
    index_len_key.append('index_keys');
    index_len_key.append(index);
    index_len_key.append(id);
    index_len_key.span()
}

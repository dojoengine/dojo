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
    // TODO: handle reapeated
    assert(keys.len() < 255, 'Too many keys');
    create(address_domain, index, id);

    let mut positions = ArrayTrait::<felt252>::new();
    positions.append(keys.len().into());

    let mut idx = 0;
    loop {
        if idx == keys.len() {
            break ();
        }
        let pos = add_key(address_domain, index, id, *keys.at(idx), idx); // key -> id
        positions.append(pos);
        idx += 1;
    };

    let index_len_key = build_index_len_key(index);

    let keys_len: u8 = keys.len().try_into().unwrap();
    storage::set_many(address_domain, build_index_item_keys(index, id), 0, positions.span());  // len of keys and positions
    storage::set_many(address_domain, build_index_item_keys(index, id), keys_len + 1, keys);   // keys
}

fn add_key(address_domain: u32, index: felt252, id: felt252, key: felt252, idx: u32) -> felt252 {
    let specific_len_key = build_index_specific_key_len(index, key);
    let specific_len = storage::get(address_domain, specific_len_key);
    let specific_key = build_index_specific_key(index, key, specific_len);

    storage::set(address_domain, specific_len_key, specific_len + 1);
    let mut val = ArrayTrait::new();
    val.append(id);
    val.append(idx.into());
    storage::set_many(address_domain, specific_key, 0, val.span());
    specific_len
}

fn delete_key(address_domain: u32, index: felt252, id: felt252, key: felt252, pos: felt252) {
    // TODO
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


    let keys_key = build_index_item_keys(index, id);
    let len = (*storage::get_many(address_domain, keys_key, 0, 1).at(0)).try_into().unwrap();
    let end_keys: u32 = len + len + 1;
    let pos_and_keys = storage::get_many(address_domain, keys_key, 1, end_keys);


    let mut idx: u32 = 0;
    loop {
        if idx == len {
            break ();
        }
        delete_key(address_domain, index, id, *pos_and_keys.at(idx + 1), *pos_and_keys.at(idx + len + 1));
        idx += 1;
    };
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
        let keys_key = build_index_item_keys(index, id);

        let len = *storage::get_many(address_domain, keys_key, 0, 1).at(0);
        let offset: u8 = (len + 1).try_into().unwrap();
        let len: u32 = (len + len + 1).try_into().unwrap();
        let keys = storage::get_many(address_domain, keys_key, offset, len);
        ids.append(id);
        all_keys.append(keys);
        idx += 1;
    };

    (ids, all_keys)
}

fn get_by_key(address_domain: u32, index: felt252, key: felt252) -> Array<felt252> {
    let mut res = ArrayTrait::new();
    let specific_len_key = build_index_specific_key_len(index, key);
    let specific_len = storage::get(address_domain, specific_len_key);
    let index_len = storage::get(address_domain, specific_len_key);

    let mut idx = 0;

    loop {
        if idx == index_len {
            break ();
        }

        let specific_key = build_index_specific_key(index, key, idx);
        let id = storage::get(address_domain, specific_key);
        res.append(id);

        idx += 1;
    };
    res
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

fn build_index_specific_key_len(index: felt252, key: felt252) -> Span<felt252> {
    let mut index_len_key = index_key_prefix();
    index_len_key.append('index_key_len');
    index_len_key.append(index);
    index_len_key.append(key);
    index_len_key.span()
}

fn build_index_specific_key(index: felt252, key: felt252, idx: felt252) -> Span<felt252> {
    let mut index_len_key = index_key_prefix();
    index_len_key.append('index_key');
    index_len_key.append(index);
    index_len_key.append(key);
    index_len_key.append(idx);
    index_len_key.span()
}
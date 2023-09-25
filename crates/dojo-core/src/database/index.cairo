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

/// Writes a new entry to the index, with given keys.
/// # Arguments 
/// * address_domain - The address domain to write to.
/// * index - The index to write to.
/// * id - The id of the entry.
/// * keys - The keys to write.the entry to. 
fn create_with_keys(address_domain: u32, index: felt252, id: felt252, keys: Span<felt252>, layout: Span<u8>) {
    // TODO: handle reapeated
    assert(keys.len() < 255, 'Too many keys');
    create(address_domain, index, id);

    let mut positions = ArrayTrait::<felt252>::new();
    let mut positions_layout = ArrayTrait::<u8>::new();
    positions.append(keys.len().into());
    positions_layout.append(8);

    let mut idx = 0;
    loop {
        if idx == keys.len() {
            break ();
        }
        let pos = add_key(address_domain, index, id, *keys.at(idx), idx); // key -> id
        positions.append(pos);
        positions_layout.append(251);
        idx += 1;
    };

    let index_len_key = build_index_len_key(index);

    let keys_len: u8 = keys.len().try_into().unwrap();
    storage::set_many(address_domain, build_index_item_keys(index, id), 0, positions.span(), positions_layout.span());  // len of keys and positions
    storage::set_many(address_domain, build_index_item_keys(index, id), keys_len + 1, keys, layout);   // keys
}

/// Adds a single key for a given id.
/// # Arguments
/// * address_domain - The address domain to write to.
/// * index - The index to write to.
/// * id - The id of the entry.
/// * key - The key to write.
/// * idx - The index of the key in the keys array.
/// # Returns
/// The position of the key in the the index.
fn add_key(address_domain: u32, index: felt252, id: felt252, key: felt252, idx: u32) -> felt252 {
    let specific_len_key = build_index_specific_key_len(index, key);
    let specific_len = storage::get(address_domain, specific_len_key);
    let specific_key = build_index_specific_key(index, key, specific_len);

    storage::set(address_domain, specific_len_key, specific_len + 1);
    let val = array![id, idx.into()].span();
    let layout = array![251, 8].span();
    storage::set_many(address_domain, specific_key, 0, val, layout);
    specific_len
}

/// Deletes a single key for a given id.
/// # Arguments
/// * address_domain - The address domain to write to.
/// * index - The index to write to.
/// * id - The id of the entry.
/// * key - The key to write.
/// * pos - The position of the key in the the index.
fn delete_key(address_domain: u32, index: felt252, id: felt252, key: felt252, pos: felt252) {
    // TODO
}

/// Deletes an entry from the main index, as well as from each of the keys.
/// # Arguments
/// * address_domain - The address domain to write to.
/// * index - The index to write to.
/// * id - The id of the entry.
/// # Returns
fn delete(address_domain: u32, index: felt252, id: felt252, keys_layout: Span<u8>) {
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
    let len = (*storage::get_many(address_domain, keys_key, 0, 1, array![8].span()).at(0)).try_into().unwrap();
    let end_keys: u32 = len + len + 1;

    let mut idx: u32 = 0;
    let mut layout = array![8];
        loop {
        if idx == len * 2 {
            break ();
        }
        else if idx < len {
            layout.append(251);
        } else {
            layout.append(*keys_layout.at(idx - len));
        }
        idx += 1;
    };

    let len_pos_and_keys = storage::get_many(address_domain, keys_key, 0, end_keys, layout.span());


    let mut idx: u32 = 0;
    loop {
        if idx == len {
            break ();
        }
        delete_key(address_domain, index, id, *len_pos_and_keys.at(idx + 1), *len_pos_and_keys.at(idx + len + 1));
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

/// Gets all ids for a given index, as well as all keys for each id.
/// # Arguments
/// * address_domain - The address domain to write to.
/// * index - The index to write to.
/// # Returns
/// * ids - The ids for the index.
fn get_with_keys(address_domain: u32, index: felt252, keys_layout: Span<u8>) -> (Array<felt252>, Array<Span<felt252>>) {
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

        let len_felt = (*storage::get_many(address_domain, keys_key, 0, 1, array![8].span()).at(0));
        let len = len_felt.try_into().unwrap();
        let mut end_keys: u64 = 8 + 251 * len.into();

        let mut layout = array![8];
        let mut layout_idx = 0;
        loop {
            if layout_idx == len * 2 {
                break ();
            }
            else if layout_idx < len {
                layout.append(251);
            } else {
                layout.append(*keys_layout.at(layout_idx - len));
                end_keys += (*keys_layout.at(layout_idx - len)).into();
            }
            layout_idx += 1;
        };

        let end: u32 = ((end_keys + 250) / 251).try_into().unwrap();

        let len_pos_and_keys = storage::get_many(address_domain, keys_key, 0, end, layout.span());

        let len = *storage::get_many(address_domain, keys_key, 0, 1, array![8].span()).at(0);
        let offset: u8 = (len + 1).try_into().unwrap();
        let len: u32 = (len + len + 1).try_into().unwrap();
        let keys = storage::get_many(address_domain, keys_key, offset, len, keys_layout);
        ids.append(id);
        all_keys.append(keys);
        idx += 1;
    };

    (ids, all_keys)
}

/// Returns all the entries that hold a giben key
/// # Arguments
/// * address_domain - The address domain to write to.
/// * index - The index to read from.
/// * key - The key return values from.
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

/// Key data about keys of a given entry.
/// # Arguments
/// * index - The index to write to.
/// * id - The id of the entry.
fn build_index_item_keys(index: felt252, id: felt252) -> Span<felt252> {
    let mut index_len_key = index_key_prefix();
    index_len_key.append('index_keys');
    index_len_key.append(index);
    index_len_key.append(id);
    index_len_key.span()
}

/// Key for a length of index for a given key.
/// # Arguments
/// * index - The index to write to.
/// * key - The key to write.
fn build_index_specific_key_len(index: felt252, key: felt252) -> Span<felt252> {
    let mut index_len_key = index_key_prefix();
    index_len_key.append('index_key_len');
    index_len_key.append(index);
    index_len_key.append(key);
    index_len_key.span()
}

/// Key for an index of a given key.
/// # Arguments
/// * index - The index to write to.
/// * key - The key to write.
/// * idx - The position in the index.
fn build_index_specific_key(index: felt252, key: felt252, idx: felt252) -> Span<felt252> {
    let mut index_len_key = index_key_prefix();
    index_len_key.append('index_key');
    index_len_key.append(index);
    index_len_key.append(key);
    index_len_key.append(idx);
    index_len_key.span()
}
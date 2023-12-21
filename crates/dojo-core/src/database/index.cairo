use array::{ArrayTrait, SpanTrait};
use traits::Into;
use option::OptionTrait;
use poseidon::poseidon_hash_span;
use serde::Serde;

use dojo::database::storage;

#[derive(Copy, Drop)]
struct WhereCondition {
    key: felt252,
    value: felt252,
}

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

/// Deletes an entry from the main index, as well as from each of the keys.
/// # Arguments
/// * address_domain - The address domain to write to.
/// * index - The index to write to.
/// * id - The id of the entry.
/// # Returns
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
    // NOTE: We leave the last element set as to not produce an unnecessary state diff.
    let replace_item_value = storage::get(address_domain, build_index_key(index, replace_item_idx));
    storage::set(address_domain, build_index_key(index, delete_item_idx), replace_item_value);
}

fn exists(address_domain: u32, index: felt252, id: felt252) -> bool {
    storage::get(address_domain, build_index_item_key(index, id)) != 0
}

fn query(address_domain: u32, table: felt252, where: Option<WhereCondition>) -> Span<felt252> {
    let mut res = ArrayTrait::new();

    match where {
        Option::Some(clause) => {
            let mut serialized = ArrayTrait::new();
            table.serialize(ref serialized);
            clause.key.serialize(ref serialized);
            let index = poseidon_hash_span(serialized.span());

            let index_len_key = build_index_len_key(index);
            let index_len = storage::get(address_domain, index_len_key);
            let mut idx = 0;

            loop {
                if idx == index_len {
                    break ();
                }
                let id = storage::get(address_domain, build_index_key(index, idx));
                res.append(id);
            }
        },

        // If no `where` clause is defined, we return all values.
        Option::None(_) => {
            let index_len_key = build_index_len_key(table);
            let index_len = storage::get(address_domain, index_len_key);
            let mut idx = 0;

            loop {
                if idx == index_len {
                    break ();
                }

                res.append(storage::get(address_domain, build_index_key(table, idx)));
                idx += 1;
            };
        }
    }

    res.span()
}

/// Returns all the entries that hold a given key
/// # Arguments
/// * address_domain - The address domain to write to.
/// * index - The index to read from.
/// * key - The key return values from.
fn get_by_key(address_domain: u32, index: felt252, key: felt252) -> Array<felt252> {
    let mut res = ArrayTrait::new();
    let specific_len_key = build_index_specific_key_len(index, key);
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

fn build_index_len_key(index: felt252) -> Span<felt252> {
    array!['dojo_index_lens', index].span()
}

fn build_index_key(index: felt252, idx: felt252) -> Span<felt252> {
    array!['dojo_indexes', index, idx].span()
}

fn build_index_item_key(index: felt252, id: felt252) -> Span<felt252> {
    array!['dojo_index_ids', index, id].span()
}

/// Key for a length of index for a given key.
/// # Arguments
/// * index - The index to write to.
/// * key - The key to write.
fn build_index_specific_key_len(index: felt252, key: felt252) -> Span<felt252> {
    array!['dojo_index_key_len', index, key].span()
}

/// Key for an index of a given key.
/// # Arguments
/// * index - The index to write to.
/// * key - The key to write.
/// * idx - The position in the index.
fn build_index_specific_key(index: felt252, key: felt252, idx: felt252) -> Span<felt252> {
    array!['dojo_index_key', index, key, idx].span()
}
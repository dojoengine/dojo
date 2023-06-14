use array::{ArrayTrait, SpanTrait};
use traits::Into;
use option::OptionTrait;
use poseidon::poseidon_hash_span;

use dojo_core::storage;

fn create(address_domain: u32, table: felt252, id: felt252) {
    if exists(address_domain, table, id) {
        return ();
    }

    let table_len_key = build_table_len_key(table);
    let table_len = storage::get(address_domain, table_len_key);
    storage::set(address_domain, build_table_id_key(table, id), table_len + 1);
    storage::set(address_domain, table_len_key, table_len + 1);
    storage::set(address_domain, build_tables_key(table, table_len), id);
}

fn delete(address_domain: u32, table: felt252, id: felt252) {
    if !exists(address_domain, table, id) {
        return ();
    }

    // let table_len = table_lens.read(table);
    // let table_idx = ids.read((table, id)) - 1;
    // ids.write((table, id), 0);
    // table_lens.write(table, table_len - 1);

    // // Replace the deleted element with the last element.
    // // NOTE: We leave the last element set as to not produce an unncessary state diff.
    // tables.write((table, table_idx), tables.read((table, table_len - 1)));
}

fn exists(address_domain: u32, table: felt252, id: felt252) -> bool {
    storage::get(address_domain, build_table_id_key(table, id)) != 0
}

fn query(address_domain: u32, table: felt252) -> Array<felt252> {
    let mut res = ArrayTrait::new();
    // let table_len = table_lens.read(table);
    // let mut idx: usize = 0;

    // loop {
    //     if idx == table_len {
    //         break ();
    //     }

    //     res.append(tables.read((table, idx)));
    //     idx += 1;
    // };

    res
}

fn build_table_len_key(table: felt252) -> Span<felt252> {
    let mut table_len_key = ArrayTrait::new();
    table_len_key.append('index');
    table_len_key.append('table_lens');
    table_len_key.append(table);
    table_len_key.span()
}

fn build_tables_key(table: felt252, len: felt252) -> Span<felt252> {
    let mut table_len_key = ArrayTrait::new();
    table_len_key.append('index');
    table_len_key.append('tables');
    table_len_key.append(table);
    table_len_key.append(len);
    table_len_key.span()
}

fn build_table_id_key(table: felt252, id: felt252) -> Span<felt252> {
    let mut table_len_key = ArrayTrait::new();
    table_len_key.append('index');
    table_len_key.append('table_lens');
    table_len_key.append(table);
    table_len_key.append(id);
    table_len_key.span()
}
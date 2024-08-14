use core::array::{ArrayTrait, SpanTrait};
use core::hash::LegacyHash;
use core::option::OptionTrait;
use core::poseidon::poseidon_hash_span;
use core::serde::Serde;
use core::traits::{Into, TryInto};

use starknet::SyscallResultTrait;

use super::storage;

const DOJO_STORAGE: felt252 = 'dojo_storage';

pub const MAX_ARRAY_LENGTH: u256 = 4_294_967_295;

/// Fill the provided array with zeroes.
///
/// # Arguments
///   * `values` - the array to fill
///   * `size` - the number of zero to append in the array
fn fill_with_zeroes(ref values: Array<felt252>, size: u32) {
    let mut i = 0;

    loop {
        if i >= size {
            break;
        }
        values.append(0);
        i += 1;
    }
}

/// Compute the internal storage key from a table selector and a key.
///
/// # Arguments
///   * `table` - the table selector
///   * `key` - a key to identify a record in the table
///
/// # Returns
///   A [`Span<felt252>`] representing an internal storage key.
fn get_storage_key(table: felt252, key: felt252) -> Span<felt252> {
    [DOJO_STORAGE, table, key].span()
}

/// Read a record from a table, with its ID and layout.
///
/// # Arguments
///   * `table` - the table selector
///   * `key` - key of the record to read
///   * `layout` - the layout of the record to read.
///
/// # Returns
///   A [`Span<felt252>`] containing the raw unpacked data of the read record.
pub fn get(table: felt252, key: felt252, layout: Span<u8>) -> Span<felt252> {
    storage::get_many(0, get_storage_key(table, key), layout).unwrap_syscall()
}

/// Write a record with its ID, layout and new value.
///
/// # Arguments
///   * `table` - the table selector
///   * `key` - key of the record to write
///   * `value` - the new raw unpacked data value of the record
///   * `layout` - the layout of the record to write.
pub fn set(table: felt252, key: felt252, value: Span<felt252>, offset: u32, layout: Span<u8>) {
    let storage_key = get_storage_key(table, key);
    storage::set_many(0, storage_key, value, offset, layout).unwrap_syscall();
}

/// delete a record from a table with its ID and layout.
///
/// # Arguments
///   * `table` - the table selector
///   * `key` - key of the record to delete
///   * `layout` - the layout of the record to delete
pub fn delete(table: felt252, key: felt252, layout: Span<u8>) {
    let mut reset_values = array![];
    fill_with_zeroes(ref reset_values, layout.len());
    set(table, key, reset_values.span(), 0, layout);
}

/// Write a part of an array nested in `value`, delimited by an offset and a size.
///
/// # Arguments
///  * `table` - the table selector
///  * `key` - key of the record to write
///  * `value` - the new raw unpacked data value of the record
///  * `offset` - the beginning of the nested array to write
///  * `array_size` - the size of the nested array to write
pub fn set_array(table: felt252, key: felt252, value: Span<felt252>, offset: u32, array_size: u32) {
    let storage_key = get_storage_key(table, key);
    storage::set_packed_array(0, storage_key, value, offset, array_size).unwrap_syscall();
}

/// Read an array.
///
/// # Arguments
///  * `table` - the table selector
///  * `key` - key of the record to write
///  * `array_size` - the size of the array to read.
///
/// # Returns
pub fn get_array(table: felt252, key: felt252, array_size: u32) -> Span<felt252> {
    let storage_key = get_storage_key(table, key);
    storage::get_packed_array(0, storage_key, array_size).unwrap_syscall()
}

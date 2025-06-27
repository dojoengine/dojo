use core::array::{ArrayTrait, SpanTrait};
use super::storage;

const DOJO_STORAGE: felt252 = 'dojo_storage';

pub const MAX_ARRAY_LENGTH: u256 = 4_294_967_295;

/// Fill the provided array with zeroes.
///
/// # Arguments
///   * `values` - the array to fill
///   * `size` - the number of zero to append in the array
#[inline(always)]
pub fn fill_with_zeroes(ref values: Array<felt252>, size: u32) {
    for _ in 0..size {
        values.append(0);
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
#[inline(always)]
fn get_storage_key(table: felt252, key: felt252) -> Span<felt252> {
    [DOJO_STORAGE, table, key].span()
}

#[inline(always)]
pub fn get_single(table: felt252, key: felt252) -> felt252 {
    storage::get(storage::DEFAULT_ADDRESS_DOMAIN, get_storage_key(table, key))
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
#[inline(always)]
pub fn get(table: felt252, key: felt252, layout: Span<u8>) -> Span<felt252> {
    storage::get_many(storage::DEFAULT_ADDRESS_DOMAIN, get_storage_key(table, key), layout)
}

#[inline(always)]
pub fn set_single(table: felt252, key: felt252, value: felt252) {
    storage::set(storage::DEFAULT_ADDRESS_DOMAIN, get_storage_key(table, key), value);
}

#[inline(always)]
pub fn delete_single(table: felt252, key: felt252) {
    set_single(table, key, 0);
}

/// Write a record with its ID, layout and new value.
///
/// # Arguments
///   * `table` - the table selector
///   * `key` - key of the record to write
///   * `value` - the new raw unpacked data value of the record
///   * `offset` - the offset in value to start writing from
///   * `layout` - the layout of the record to write.
#[inline(always)]
pub fn set(table: felt252, key: felt252, value: Span<felt252>, offset: u32, layout: Span<u8>) {
    let storage_key = get_storage_key(table, key);
    storage::set_many(storage::DEFAULT_ADDRESS_DOMAIN, storage_key, value, offset, layout);
}

/// delete a record from a table with its ID and layout.
///
/// # Arguments
///   * `table` - the table selector
///   * `key` - key of the record to delete
///   * `layout` - the layout of the record to delete
#[inline(always)]
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
#[inline(always)]
pub fn set_array(table: felt252, key: felt252, value: Span<felt252>, offset: u32, array_size: u32) {
    let storage_key = get_storage_key(table, key);
    storage::set_packed_array(
        storage::DEFAULT_ADDRESS_DOMAIN, storage_key, value, offset, array_size,
    );
}

/// Read an array.
///
/// # Arguments
///  * `table` - the table selector
///  * `key` - key of the record to write
///  * `array_size` - the size of the array to read.
///
/// # Returns
#[inline(always)]
pub fn get_array(table: felt252, key: felt252, array_size: u32) -> Span<felt252> {
    let storage_key = get_storage_key(table, key);
    storage::get_packed_array(storage::DEFAULT_ADDRESS_DOMAIN, storage_key, array_size)
}

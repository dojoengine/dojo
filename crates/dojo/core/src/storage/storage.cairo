use core::array::{ArrayTrait, SpanTrait};
use core::num::traits::OverflowingAdd;
use core::poseidon::poseidon_hash_span;
use core::traits::Into;
use starknet::SyscallResultTrait;
use starknet::storage_access::{
    StorageAddress, StorageBaseAddress, storage_address_from_base,
    storage_address_from_base_and_offset, storage_base_address_from_felt252,
};
use starknet::syscalls::{storage_read_syscall, storage_write_syscall};
use super::packing::{calculate_packed_size, pack, unpack};

pub const DEFAULT_ADDRESS_DOMAIN: u32 = 0;

#[inline(always)]
pub fn next_index_in_chunk(
    ref index_in_chunk: u8,
    ref chunk: felt252,
    ref chunk_base: StorageBaseAddress,
    base_address: StorageAddress,
) {
    let (sum, has_overflowed) = OverflowingAdd::overflowing_add(index_in_chunk, 1);
    match has_overflowed {
        false => { index_in_chunk = sum; },
        true => {
            index_in_chunk = 0;
            chunk += 1;
            chunk_base = chunk_segment_pointer(base_address, chunk);
        },
    }
}

#[inline(always)]
pub fn get(address_domain: u32, keys: Span<felt252>) -> felt252 {
    let base = storage_base_address_from_felt252(poseidon_hash_span(keys));
    storage_read_syscall(address_domain, storage_address_from_base(base)).unwrap_syscall()
}

/// Read a raw value defined by its layout and identified by its keys.
///
/// # Arguments
///  * `address_domain` - the address domain to use
///  * `keys` - the keys of the value to read
///  * `layout` - the layout of the value to read
///
/// # Returns
///  * `Span<felt252>` - the raw value read from the storage
///
pub fn get_many(address_domain: u32, keys: Span<felt252>, mut layout: Span<u8>) -> Span<felt252> {
    let base = storage_base_address_from_felt252(poseidon_hash_span(keys));
    let base_address = storage_address_from_base(base);

    let mut packed = ArrayTrait::new();

    let mut layout_calculate = layout;
    let len: usize = calculate_packed_size(ref layout_calculate);

    let mut chunk = 0;
    let mut chunk_base = base;
    let mut index_in_chunk = 0_u8;

    for _ in 0..len {
        let value = storage_read_syscall(
            address_domain, storage_address_from_base_and_offset(chunk_base, index_in_chunk),
        )
            .unwrap_syscall();

        packed.append(value);

        next_index_in_chunk(ref index_in_chunk, ref chunk, ref chunk_base, base_address);
    }

    let mut packed_span = packed.span();
    let mut unpacked = ArrayTrait::new();

    unpack(ref unpacked, ref packed_span, ref layout);

    unpacked.span()
}


/// Write a one-felt value identified by its keys.
///
/// # Arguments
///  * `address_domain` - the address domain to use
///  * `keys` - the keys of the value to write
///  * `value` - the value to write
///
#[inline(always)]
pub fn set(address_domain: u32, keys: Span<felt252>, value: felt252) {
    let base = storage_base_address_from_felt252(poseidon_hash_span(keys));
    storage_write_syscall(address_domain, storage_address_from_base(base), value).unwrap_syscall();
}

/// Write a raw value defined by its layout and identified by its keys.
///
/// # Arguments
///  * `address_domain` - the address domain to use
///  * `keys` - the keys of the value to write
///  * `unpacked` - the raw value to write
///  * `offset` - the starting point from where to extract raw value from the `unpacked` array.
///  * `layout` - the layout of the value to write
pub fn set_many(
    address_domain: u32,
    keys: Span<felt252>,
    mut unpacked: Span<felt252>,
    offset: u32,
    mut layout: Span<u8>,
) {
    let base = storage_base_address_from_felt252(poseidon_hash_span(keys));
    let base_address = storage_address_from_base(base);

    let mut packed = ArrayTrait::new();
    pack(ref packed, ref unpacked, offset, ref layout);

    let mut chunk = 0;
    let mut chunk_base = base;
    let mut index_in_chunk = 0_u8;

    for value in packed {
        storage_write_syscall(
            address_domain,
            storage_address_from_base_and_offset(chunk_base, index_in_chunk),
            value.into(),
        )
            .unwrap_syscall();

        next_index_in_chunk(ref index_in_chunk, ref chunk, ref chunk_base, base_address);
    }
}

/// Write a raw array value identified by its keys.
///
/// # Arguments
///  * `address_domain` - the address domain to use
///  * `keys` - the keys of the value to write
///  * `data` - the raw value to write
///  * `offset` - the starting point from where to extract raw value from the `data` array.
///  * `array_size` - the size of the array to write.
pub fn set_packed_array(
    address_domain: u32, keys: Span<felt252>, mut data: Span<felt252>, offset: u32, array_size: u32,
) {
    // write data+offset by chunk of 256 felts
    let base = storage_base_address_from_felt252(poseidon_hash_span(keys));
    let base_address = storage_address_from_base(base);

    let mut chunk = 0;
    let mut chunk_base = base;
    let mut index_in_chunk = 0_u8;

    for i in offset..offset + array_size {
        let curr_value = *data.at(i);

        storage_write_syscall(
            address_domain,
            storage_address_from_base_and_offset(chunk_base, index_in_chunk),
            curr_value,
        )
            .unwrap_syscall();

        next_index_in_chunk(ref index_in_chunk, ref chunk, ref chunk_base, base_address);
    }
}

/// Read a raw array value identified by its keys.
///
/// # Arguments
///  * `address_domain` - the address domain to use
///  * `keys` - the keys of the value to read
///  * `array_size` - the size of the array to read.
///
pub fn get_packed_array(
    address_domain: u32, keys: Span<felt252>, array_size: u32,
) -> Span<felt252> {
    if array_size == 0 {
        return [].span();
    }

    let base = storage_base_address_from_felt252(poseidon_hash_span(keys));
    let base_address = storage_address_from_base(base);

    let mut packed = ArrayTrait::new();

    let mut chunk = 0;
    let mut chunk_base = base;
    let mut index_in_chunk = 0_u8;

    loop {
        let value = storage_read_syscall(
            address_domain, storage_address_from_base_and_offset(chunk_base, index_in_chunk),
        )
            .unwrap_syscall();

        packed.append(value);

        // Verify first the length to avoid computing the new chunk segment
        // if not required.
        if packed.len() == array_size {
            break;
        }

        next_index_in_chunk(ref index_in_chunk, ref chunk, ref chunk_base, base_address);
    }

    packed.span()
}

#[inline(always)]
fn chunk_segment_pointer(address: StorageAddress, chunk: felt252) -> StorageBaseAddress {
    let p = poseidon_hash_span([address.into(), chunk, 'DojoStorageChunk'].span());
    storage_base_address_from_felt252(p)
}

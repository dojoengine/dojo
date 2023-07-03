use starknet::{ClassHash, ContractAddress};
use array::ArrayTrait;

trait Packable<T> {
    fn pack(self: @T, ref packing: felt252, packing_offset: u8, ref packed: Array<felt252>);
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, unpacking_offset: u8, ref unpacked: Array<T>);
    fn size() -> usize;
}

// impl TPackable<T, impl TSerde: Serde<T>, impl TPackable: Packable<T>> of Packable<T> {
//     fn pack(self: @T, ref packed: Array<felt252>) {
//         TSerde::serialize(self, ref packed);
//     }
//     fn unpack(ref packed: Span<felt252>) -> Option<T> {
//         TSerde::deserialize(ref packed)
//     }
//     fn size() -> usize {
//         TPackable::size()
//     }
// }

impl PackableFelt252 of Packable<felt252> {
    #[inline(always)]
    fn pack(self: @felt252, ref packing: felt252, packing_offset: u8, ref packed: Array<felt252>) {
        
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, unpacking_offset: u8, ref unpacked: Array<felt252>) {

    }
    #[inline(always)]
    fn size() -> usize {
        252
    }
}

impl PackableBool of Packable<bool> {
    #[inline(always)]
    fn pack(self: @bool, ref packing: felt252, packing_offset: u8, ref packed: Array<felt252>) {
        
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, unpacking_offset: u8, ref unpacked: Array<bool>) {

    }
    #[inline(always)]
    fn size() -> usize {
        1
    }
}

impl PackableU8 of Packable<u8> {
    #[inline(always)]
    fn pack(self: @u8, ref packing: felt252, packing_offset: u8, ref packed: Array<felt252>) {
        
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, unpacking_offset: u8, ref unpacked: Array<u8>) {

    }
    #[inline(always)]
    fn size() -> usize {
        8
    }
}

impl PackableU16 of Packable<u16> {
    #[inline(always)]
    fn pack(self: @u16, ref packing: felt252, packing_offset: u8, ref packed: Array<felt252>) {
        
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, unpacking_offset: u8, ref unpacked: Array<u16>) {

    }
    #[inline(always)]
    fn size() -> usize {
        16
    }
}

impl PackableU32 of Packable<u32> {
    #[inline(always)]
    fn pack(self: @u32, ref packing: felt252, packing_offset: u8, ref packed: Array<felt252>) {
        
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, unpacking_offset: u8, ref unpacked: Array<u32>) {

    }
    #[inline(always)]
    fn size() -> usize {
        32
    }
}

impl PackableU64 of Packable<u64> {
    #[inline(always)]
    fn pack(self: @u64, ref packing: felt252, packing_offset: u8, ref packed: Array<felt252>) {
        
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, unpacking_offset: u8, ref unpacked: Array<u64>) {

    }
    #[inline(always)]
    fn size() -> usize {
        64
    }
}

impl PackableU128 of Packable<u128> {
    #[inline(always)]
    fn pack(self: @u128, ref packing: felt252, packing_offset: u8, ref packed: Array<felt252>) {
        
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, unpacking_offset: u8, ref unpacked: Array<u128>) {

    }
    #[inline(always)]
    fn size() -> usize {
        128
    }
}

impl PackableContractAddress of Packable<ContractAddress> {
    #[inline(always)]
    fn pack(self: @ContractAddress, ref packing: felt252, packing_offset: u8, ref packed: Array<felt252>) {
        
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, unpacking_offset: u8, ref unpacked: Array<ContractAddress>) {

    }
    #[inline(always)]
    fn size() -> usize {
        252
    }
}

impl PackableClassHash of Packable<ClassHash> {
    #[inline(always)]
    fn pack(self: @ClassHash, ref packing: felt252, packing_offset: u8, ref packed: Array<felt252>) {
        
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, unpacking_offset: u8, ref unpacked: Array<ClassHash>) {

    }
    #[inline(always)]
    fn size() -> usize {
        252
    }
}

/// Pack the proposal fields into a single felt252.
fn pack(self: @felt252, size: u8, ref packing: felt252, mut packing_offset: u8, ref packed: Array<felt252>) -> u8 {
    let remaining_bits = 252 - packing_offset;

    if remaining_bits < size {
        let first_part = self & ((1 << remaining_bits) - 1);
        let second_part = self >> remaining_bits;

        // Pack the first part into the current felt
        packing = packing | (first_part << packing_offset);
        packed.append(packing);

        // Start a new felt and pack the second part into it
        packing = second_part;
        packing_offset = size - remaining_bits;
    } else {
        // Pack the data into the current felt
        packing = packing | (self << packing_offset);
        packing_offset = packing_offset + size;
    }

    packing_offset
}

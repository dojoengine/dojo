use starknet::{ClassHash, ContractAddress};
use array::{ArrayTrait, SpanTrait};
use traits::{Into, TryInto};
use integer::{U256BitAnd, U256BitOr, U256BitXor, upcast, downcast, BoundedInt};
use option::OptionTrait;

const PACKING_MAX_BITS: u8 = 251;

fn pack(
    ref packed: Array<felt252>, ref unpacked: Span<felt252>, offset: u32, ref layout: Span<u8>
) {
    assert((unpacked.len() - offset) >= layout.len(), 'mismatched input lens');
    let mut packing: felt252 = 0x0;
    let mut internal_offset: u8 = 0x0;
    let mut index = offset;
    loop {
        match layout.pop_front() {
            Option::Some(layout) => {
                pack_inner(
                    unpacked.at(index), *layout, ref packing, ref internal_offset, ref packed
                );
            },
            Option::None(_) => { break; }
        };

        index += 1;
    };
    packed.append(packing);
}

fn calculate_packed_size(ref layout: Span<u8>) -> usize {
    let mut size = 1;
    let mut partial = 0_usize;

    loop {
        match layout.pop_front() {
            Option::Some(item) => {
                let item_size: usize = (*item).into();
                partial += item_size;
                if (partial > PACKING_MAX_BITS.into()) {
                    size += 1;
                    partial = item_size;
                }
            },
            Option::None(_) => { break; }
        };
    };

    size
}

fn unpack(ref unpacked: Array<felt252>, ref packed: Span<felt252>, ref layout: Span<u8>) {
    let mut unpacking: felt252 = 0x0;
    let mut offset: u8 = PACKING_MAX_BITS;
    loop {
        match layout.pop_front() {
            Option::Some(s) => {
                match unpack_inner(*s, ref packed, ref unpacking, ref offset) {
                    Option::Some(u) => { unpacked.append(u); },
                    Option::None(_) => {
                        // Layout value was successfully poped,
                        // we are then expecting an unpacked value.
                        panic_with_felt252('Unpack inner failed');
                    }
                }
            },
            Option::None(_) => { break; }
        };
    }
}

/// Pack the proposal fields into a single felt252.
fn pack_inner(
    self: @felt252,
    size: u8,
    ref packing: felt252,
    ref packing_offset: u8,
    ref packed: Array<felt252>
) {
    assert(packing_offset <= PACKING_MAX_BITS, 'Invalid packing offset');
    assert(size <= PACKING_MAX_BITS, 'Invalid layout size');

    // Cannot use all 252 bits because some bit arrangements (eg. 11111...11111) are not valid felt252 values. 
    // Thus only 251 bits are used.                               ^-252 times-^
    // One could optimize by some conditional alligment mechanism, but it would be an at most 1/252 space-wise improvement.
    let remaining_bits: u8 = (PACKING_MAX_BITS - packing_offset).into();

    // If we have less remaining bits than the current item size,
    // Finalize the current `packing`felt and move to the next felt.
    if remaining_bits < size {
        packed.append(packing);
        packing = *self;
        packing_offset = size;
        return;
    }

    // Easier to work on u256 rather than felt252.
    let self_256: u256 = (*self).into();

    // Pack item into the `packing` felt.
    let mut packing_256: u256 = packing.into();
    packing_256 = packing_256 | shl(self_256, packing_offset);
    packing = packing_256.try_into().unwrap();
    packing_offset = packing_offset + size;
}

fn unpack_inner(
    size: u8, ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8
) -> Option<felt252> {
    let remaining_bits: u8 = (PACKING_MAX_BITS - unpacking_offset).into();

    // If less remaining bits than size, we move to the next
    // felt for unpacking.
    if remaining_bits < size {
        match packed.pop_front() {
            Option::Some(val) => {
                unpacking = *val;
                unpacking_offset = size;

                // If we are unpacking a full felt.
                if (size == PACKING_MAX_BITS) {
                    return Option::Some(unpacking);
                }

                let val_256: u256 = (*val).into();
                let result = val_256 & (shl(1, size) - 1);
                return result.try_into();
            },
            Option::None(()) => { return Option::None(()); },
        }
    }

    let mut unpacking_256: u256 = unpacking.into();
    let result = (shl(1, size) - 1) & shr(unpacking_256, unpacking_offset);
    unpacking_offset = unpacking_offset + size;
    return result.try_into();
}

fn fpow(x: u256, n: u8) -> u256 {
    if x.is_zero() {
        panic_with_felt252('base 0 not allowed in fpow');
    }

    let y = x;
    if n == 0 {
        return 1;
    }
    if n == 1 {
        return x;
    }
    let double = fpow(y * x, n / 2);
    if (n % 2) == 1 {
        return x * double;
    }
    return double;
}

fn shl(x: u256, n: u8) -> u256 {
    x * fpow(2, n)
}

fn shr(x: u256, n: u8) -> u256 {
    x / fpow(2, n)
}

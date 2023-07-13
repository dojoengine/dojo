use core::clone::Clone;
use array::{ArrayTrait, SpanTrait};
use starknet::{ClassHash, ContractAddress};
use traits::{Into, TryInto};
use integer::{U256BitAnd, U256BitOr, U256BitXor, upcast, downcast, BoundedInt};
use option::OptionTrait;

#[derive(Copy, Drop)]
struct LayoutItem {
    value: felt252,
    size: u8
}

// The chain of transformations would be as folows:
// struct -to_unpacked()-> 
// unpacked array -pack()-> 
// packed array -storage::set()-> 
// syscall saved -storage::get()->
// packed array -unpack()-> 
// unpacked array -get_layout() & from_unpacked()-> struct
trait StorageLayoutTrait<T> {
    // This definition, although not explicitly recursive is recursive in practice.
    // Individual implementations of particular structs would recursively call to_unpacked()
    // This way a layout array can be constructed
    // Called when setting
    fn to_layout(self: @T) -> Array<LayoutItem>;
    fn to_unpacked(self: @T) -> Array<LayoutItem>;

    // Called when getting
    fn get_layout() -> Span<u8>;
    fn from_unpacked(items: Span<felt252>) -> T;
}

fn layout_length(ref layout: Span<u8>) -> u32 {
    let mut sum: u32 = 0;
    loop {
        match layout.pop_front() {
            Option::Some(i) => {
                sum = sum + (*i).into();
            },
            Option::None(_) => {
                break;
            }
        };
    };
    sum
}

fn pack(ref unpacked: Array<LayoutItem>) -> Span<felt252> {
    let mut packed: Array<felt252> = ArrayTrait::new();
    let mut packing: felt252 = 0x0;
    let mut offset: u8 = 0x0;
    loop {
        match unpacked.pop_front() {
            Option::Some(s) => {
                pack_util(@s.value, s.size, ref packing, ref offset, ref packed);
            },
            Option::None(_) => {
                break packed.span();
            }
        };
    }
}

fn unpack(ref packed: Span<felt252>, ref layout: Span<u8>) -> Option<Span<felt252>> {
    let mut unpacked: Array<felt252> = ArrayTrait::new();
    let mut unpacking: felt252 = 0x0;
    let mut offset: u8 = 251;
    loop {
        match layout.pop_front() {
            Option::Some(s) => {
                match unpack_util(*s, ref packed, ref unpacking, ref offset) {
                    Option::Some(u) => {
                        unpacked.append(u);
                    },
                    Option::None(_) => {
                        break Option::None(());
                    }
                }
            },
            Option::None(_) => {
                break Option::Some(unpacked.span());
            }
        };
    }
}

/// Pack the proposal fields into a single felt252.
fn pack_util(
    self: @felt252,
    size: u8,
    ref packing: felt252,
    ref packing_offset: u8,
    ref packed: Array<felt252>
) {
    // Easier to work on u256 rather than felt252.
    let self_256: u256 = (*self).into();

    // Cannot use all 252 bits because some bit arrangements (eg. 11111...11111) are not valid felt252 values. 
    // Thus only 251 bits are used.                               ^-252 times-^
    // One could optimize by some conditional alligment mechanism, but it would be an at most 1/252 space-wise improvement.
    let remaining_bits: u8 = (251 - packing_offset).into();

    let mut packing_256: u256 = packing.into();

    if remaining_bits < size {
        let first_part = self_256 & (shl(1, remaining_bits) - 1);
        let second_part = shr(self_256, remaining_bits);

        // Pack the first part into the current felt
        packing_256 = packing_256 | shl(first_part, packing_offset);
        packed.append(packing_256.try_into().unwrap());

        // Start a new felt and pack the second part into it
        packing = second_part.try_into().unwrap();
        packing_offset = size - remaining_bits;
    } else {
        // Pack the data into the current felt
        packing_256 = packing_256 | shl(self_256, packing_offset);
        packing = packing_256.try_into().unwrap();
        packing_offset = packing_offset + size;
    }
}

fn unpack_util(
    size: u8, ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8
) -> Option<felt252> {
    let remaining_bits: u8 = (251 - unpacking_offset).into();

    let mut unpacking_256: u256 = unpacking.into();

    if remaining_bits < size {
        match packed.pop_front() {
            Option::Some(val) => {
                let val_256: u256 = (*val).into();

                // Get the first part
                let first_part = shr(unpacking_256, unpacking_offset);
                // Size of the remaining part
                let second_size = size - remaining_bits;
                let second_part = val_256 & (shl(1, second_size) - 1);
                // Move the second part so it fits alongside the first part
                let result = first_part | shl(second_part, remaining_bits);

                unpacking = *val;
                unpacking_offset = second_size;
                return result.try_into();
            },
            Option::None(()) => {
                return Option::None(());
            },
        }
    } else {
        let result = (shl(1, size) - 1) & shr(unpacking_256, unpacking_offset);
        unpacking_offset = unpacking_offset + size;
        return result.try_into();
    }
}

fn fpow(x: u256, n: u8) -> u256 {
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

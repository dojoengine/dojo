use starknet::{ClassHash, ContractAddress};
use array::{ArrayTrait, SpanTrait};
use traits::{Into, TryInto};
use integer::{U256BitAnd, U256BitOr, U256BitXor, upcast, downcast, BoundedInt};
use option::OptionTrait;


/// Pack the proposal fields into a single felt252.
fn pack(self: @felt252, size: u8, ref packing: felt252, ref packing_offset: u8, ref packed: Array<felt252>) {
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

fn unpack(size: u8, ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) -> Option<felt252> {
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

trait Packable<T> {
    fn pack(self: @T, ref packing: felt252, ref packing_offset: u8, ref packed: Array<felt252>);
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) -> Option<T>;
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
    fn pack(self: @felt252, ref packing: felt252, ref packing_offset: u8, ref packed: Array<felt252>) {
        pack(
            self, 
            Packable::<felt252>::size().try_into().unwrap(), 
            ref packing, ref packing_offset, ref packed
        )
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) -> Option<felt252> {
        unpack(
            Packable::<felt252>::size().try_into().unwrap(), 
            ref packed, ref unpacking, ref unpacking_offset
        )
    }
    #[inline(always)]
    fn size() -> usize {
        252
    }
}

impl PackableBool of Packable<bool> {
    #[inline(always)]
    fn pack(self: @bool, ref packing: felt252, ref packing_offset: u8, ref packed: Array<felt252>) {
        let self_felt252: felt252 = {
            if *self {
                1_felt252
            } else {
                0_felt252
            }
        };
        pack(
            @self_felt252, Packable::<bool>::size().try_into().unwrap(), 
            ref packing, ref packing_offset, ref packed
        )
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) -> Option<bool> {
        match unpack(
            Packable::<bool>::size().try_into().unwrap(), 
            ref packed, ref unpacking, ref unpacking_offset
        ){
            Option::Some(val) => {
                if val == 0_felt252 {
                    Option::Some(false)
                } else if val == 1_felt252 {
                    Option::Some(true)
                } else {
                    Option::None(())
                }
            },
            Option::None(()) => Option::None(()),
        }
    }
    #[inline(always)]
    fn size() -> usize {
        1
    }
}

impl PackableU8 of Packable<u8> {
    #[inline(always)]
    fn pack(self: @u8, ref packing: felt252, ref packing_offset: u8, ref packed: Array<felt252>) {
        pack(
            @(*self).into(), Packable::<u8>::size().try_into().unwrap(), 
            ref packing, ref packing_offset, ref packed
        )
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) -> Option<u8> {
        match unpack(Packable::<u8>::size().try_into().unwrap(), ref packed, ref unpacking, ref unpacking_offset){
            Option::Some(val) => val.try_into(),
            Option::None(()) => Option::None(()),
        }
    }
    #[inline(always)]
    fn size() -> usize {
        8
    }
}

impl PackableU16 of Packable<u16> {
    #[inline(always)]
    fn pack(self: @u16, ref packing: felt252, ref packing_offset: u8, ref packed: Array<felt252>) {
        pack(
            @(*self).into(), 
            Packable::<u16>::size().try_into().unwrap(), 
            ref packing, ref packing_offset, ref packed
        )
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) -> Option<u16> {
        match unpack(
            Packable::<u16>::size().try_into().unwrap(), 
            ref packed, ref unpacking, ref unpacking_offset
        ){
            Option::Some(val) => val.try_into(),
            Option::None(()) => Option::None(()),
        }
    }
    #[inline(always)]
    fn size() -> usize {
        16
    }
}

impl PackableU32 of Packable<u32> {
    #[inline(always)]
    fn pack(self: @u32, ref packing: felt252, ref packing_offset: u8, ref packed: Array<felt252>) {
        pack(
            @(*self).into(), 
            Packable::<u32>::size().try_into().unwrap(), 
            ref packing, ref packing_offset, ref packed
        )
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) -> Option<u32> {
        match unpack(
            Packable::<u32>::size().try_into().unwrap(), 
            ref packed, ref unpacking, ref unpacking_offset
        ) {
            Option::Some(val) => val.try_into(),
            Option::None(()) => Option::None(()),
        }
    }
    #[inline(always)]
    fn size() -> usize {
        32
    }
}

impl PackableU64 of Packable<u64> {
    #[inline(always)]
    fn pack(self: @u64, ref packing: felt252, ref packing_offset: u8, ref packed: Array<felt252>) {
        pack(
            @(*self).into(), 
            Packable::<u64>::size().try_into().unwrap(), 
            ref packing, ref packing_offset, ref packed
        )
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) -> Option<u64> {
        match unpack(
            Packable::<u64>::size().try_into().unwrap(), 
            ref packed, ref unpacking, ref unpacking_offset
        ) {
            Option::Some(val) => val.try_into(),
            Option::None(()) => Option::None(()),
        }
    }
    #[inline(always)]
    fn size() -> usize {
        64
    }
}

impl PackableU128 of Packable<u128> {
    #[inline(always)]
    fn pack(self: @u128, ref packing: felt252, ref packing_offset: u8, ref packed: Array<felt252>) {
        pack(
            @(*self).into(), 
            Packable::<u128>::size().try_into().unwrap(), 
            ref packing, ref packing_offset, ref packed
        )
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) -> Option<u128> {
        match unpack(
            Packable::<u128>::size().try_into().unwrap(), 
            ref packed, ref unpacking, ref unpacking_offset
        ) {
            Option::Some(val) => val.try_into(),
            Option::None(()) => Option::None(()),
        }
    }
    #[inline(always)]
    fn size() -> usize {
        128
    }
}

impl PackableContractAddress of Packable<ContractAddress> {
    #[inline(always)]
    fn pack(self: @ContractAddress, ref packing: felt252, ref packing_offset: u8, ref packed: Array<felt252>) {
        let self_felt252: felt252 = (*self).into();
        self_felt252.pack(ref packing, ref packing_offset, ref packed)
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) -> Option<ContractAddress> {
        match Packable::<felt252>::unpack(ref packed, ref unpacking, ref unpacking_offset) {
            Option::Some(val) => val.try_into(),
            Option::None(()) => Option::None(()),
        }
    }
    #[inline(always)]
    fn size() -> usize {
        252
    }
}

impl PackableClassHash of Packable<ClassHash> {
    #[inline(always)]
    fn pack(self: @ClassHash, ref packing: felt252, ref packing_offset: u8, ref packed: Array<felt252>) {
        let self_felt252: felt252 = (*self).into();
        self_felt252.pack(ref packing, ref packing_offset, ref packed)
    }
    #[inline(always)]
    fn unpack(ref packed: Span<felt252>, ref unpacking: felt252, ref unpacking_offset: u8) -> Option<ClassHash> {
        match Packable::<felt252>::unpack(ref packed, ref unpacking, ref unpacking_offset) {
            Option::Some(val) => val.try_into(),
            Option::None(()) => Option::None(()),
        }
    }
    #[inline(always)]
    fn size() -> usize {
        252
    }
}

fn fpow(x: u256, n: u8) -> u256 {
    let y = x;
    if n == 0 { return 1; } 
    if n == 1 { return x; }
    let double = fpow(y * x, n / 2);
    if (n % 2) == 1 { return x * double; } 
    return double;
    
}

fn shl(x: u256, n: u8) -> u256 {
    x * fpow(2, n)
}

fn shr(x: u256, n: u8) -> u256 {
    x / fpow(2, n)
}

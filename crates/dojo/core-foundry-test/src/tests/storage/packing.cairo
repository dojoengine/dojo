use core::array::ArrayTrait;
use core::option::OptionTrait;
use core::traits::{Into, TryInto};

use dojo::storage::packing::{
    shl, shr, fpow, pack, unpack, pack_inner, unpack_inner, calculate_packed_size, pow2_const
};

#[test]
#[available_gas(9000000)]
fn test_bit_fpow() {
    assert(
        fpow(
            2, 250
        ) == 1809251394333065553493296640760748560207343510400633813116524750123642650624_u256,
        ''
    )
}


#[test]
fn test_bit_pow2_const() {
    assert(
        pow2_const(
            250
        ) == 1809251394333065553493296640760748560207343510400633813116524750123642650624_u256,
        ''
    )
}

#[test]
#[available_gas(9000000)]
fn test_bit_shift() {
    assert(1 == shl(1, 0), 'left == right');
    assert(1 == shr(1, 0), 'left == right');

    assert(16 == shl(1, 4), 'left == right');
    assert(1 == shr(16, 4), 'left == right');

    assert(shr(shl(1, 251), 251) == 1, 'left == right')
}

#[test]
#[available_gas(9000000)]
fn test_pack_unpack_single() {
    let mut packed = ArrayTrait::new();
    let mut packing: felt252 = 0;
    let mut offset = 0;
    pack_inner(@18, 251, ref packing, ref offset, ref packed);
    packed.append(packing);

    assert(*packed.at(0) == 18, 'Packing single value');

    let mut unpacking: felt252 = packed.pop_front().unwrap();
    let mut un_offset = 0;
    let mut packed_span = packed.span();

    let result = unpack_inner(251, ref packed_span, ref unpacking, ref un_offset).unwrap();
    assert(result == 18, 'Unpacked equals packed');
}

#[test]
#[available_gas(9000000)]
fn test_pack_unpack_felt252_u128() {
    let mut packed = ArrayTrait::new();
    let mut packing: felt252 = 0;
    let mut offset = 0;
    pack_inner(@1337, 128, ref packing, ref offset, ref packed);
    pack_inner(@420, 251, ref packing, ref offset, ref packed);
    packed.append(packing);

    let mut unpacking: felt252 = packed.pop_front().unwrap();
    let mut un_offset = 0;
    let mut packed_span = packed.span();

    assert(
        unpack_inner(128, ref packed_span, ref unpacking, ref un_offset).unwrap() == 1337,
        'Types u8'
    );
    assert(
        unpack_inner(252, ref packed_span, ref unpacking, ref un_offset).unwrap() == 420, 'Types u8'
    );
}

#[test]
#[available_gas(100000000)]
fn test_pack_multiple() {
    let mut packed = ArrayTrait::new();
    let mut packing: felt252 = 0;
    let mut offset = 0;

    let mut i: u32 = 0;
    loop {
        if i >= 20 {
            break;
        }
        pack_inner(@i.into(), 32, ref packing, ref offset, ref packed);
        i += 1;
    };
    packed.append(packing);

    assert(
        *packed.at(0) == 0x6000000050000000400000003000000020000000100000000, 'Packed multiple 0'
    );
    assert(
        *packed.at(1) == 0xd0000000c0000000b0000000a000000090000000800000007, 'Packed multiple 1'
    );
    assert(*packed.at(2) == 0x130000001200000011000000100000000f0000000e, 'Packed multiple 2');
}

#[test]
#[available_gas(500000000)]
fn test_pack_unpack_multiple() {
    let mut packed = ArrayTrait::new();
    let mut packing: felt252 = 0;
    let mut offset = 0;

    let mut i: u8 = 0;
    loop {
        if i >= 40 {
            break;
        }
        let mut j: u32 = i.into();
        j = (j + 3) * j;

        pack_inner(@i.into(), 8, ref packing, ref offset, ref packed);
        pack_inner(@j.into(), 32, ref packing, ref offset, ref packed);

        i += 1;
    };
    packed.append(packing);

    let mut unpacking: felt252 = packed.pop_front().unwrap();
    let mut un_offset = 0;
    let mut packed_span = packed.span();

    i = 0;
    loop {
        if i >= 40 {
            break;
        }
        let result_i = unpack_inner(8, ref packed_span, ref unpacking, ref un_offset).unwrap();
        let result_j = unpack_inner(32, ref packed_span, ref unpacking, ref un_offset).unwrap();

        let mut j: u32 = i.into();
        j = (j + 3) * j;

        assert(result_i.try_into().unwrap() == i, 'Unpacked equals packed');
        assert(result_j.try_into().unwrap() == j, 'Unpacked equals packed');
        i += 1;
    };
}

#[test]
#[available_gas(500000000)]
fn test_pack_unpack_types() {
    let mut packed = ArrayTrait::new();
    let mut packing: felt252 = 0;
    let mut offset = 0;

    pack_inner(@3, 8, ref packing, ref offset, ref packed);
    pack_inner(@14, 16, ref packing, ref offset, ref packed);
    pack_inner(@59, 32, ref packing, ref offset, ref packed);
    pack_inner(@26, 64, ref packing, ref offset, ref packed);
    pack_inner(@53, 128, ref packing, ref offset, ref packed);
    pack_inner(@58, 251, ref packing, ref offset, ref packed);
    pack_inner(@false.into(), 1, ref packing, ref offset, ref packed);

    let contract_address = starknet::contract_address_const::<3>();
    pack_inner(@contract_address.into(), 251, ref packing, ref offset, ref packed);
    let class_hash = starknet::class_hash::class_hash_const::<1337>();
    pack_inner(@class_hash.into(), 251, ref packing, ref offset, ref packed);

    packed.append(packing);

    let mut unpacking: felt252 = packed.pop_front().unwrap();
    let mut un_offset = 0;
    let mut packed_span = packed.span();

    assert(
        unpack_inner(8, ref packed_span, ref unpacking, ref un_offset)
            .unwrap()
            .try_into()
            .unwrap() == 3_u8,
        'Types u8'
    );
    assert(
        unpack_inner(16, ref packed_span, ref unpacking, ref un_offset)
            .unwrap()
            .try_into()
            .unwrap() == 14_u16,
        'Types u16'
    );
    assert(
        unpack_inner(32, ref packed_span, ref unpacking, ref un_offset)
            .unwrap()
            .try_into()
            .unwrap() == 59_u32,
        'Types u32'
    );
    assert(
        unpack_inner(64, ref packed_span, ref unpacking, ref un_offset)
            .unwrap()
            .try_into()
            .unwrap() == 26_u64,
        'Types u64'
    );
    assert(
        unpack_inner(128, ref packed_span, ref unpacking, ref un_offset)
            .unwrap()
            .try_into()
            .unwrap() == 53_u128,
        'Types u128'
    );
    assert(
        unpack_inner(251, ref packed_span, ref unpacking, ref un_offset).unwrap() == 58_felt252,
        'Types felt252'
    );
    assert(
        unpack_inner(1, ref packed_span, ref unpacking, ref un_offset).unwrap() == false.into(),
        'Types bool'
    );
    assert(
        unpack_inner(251, ref packed_span, ref unpacking, ref un_offset)
            .unwrap()
            .try_into()
            .unwrap() == contract_address,
        'Types ContractAddress'
    );
    assert(
        unpack_inner(251, ref packed_span, ref unpacking, ref un_offset)
            .unwrap()
            .try_into()
            .unwrap() == class_hash,
        'Types ClassHash'
    );
}

#[test]
#[available_gas(9000000)]
fn test_inner_pack_unpack_u256_single() {
    let input: u256 = 2000;
    let mut packed = ArrayTrait::new();
    let mut packing: felt252 = 0;
    let mut offset = 0;
    pack_inner(@input.low.into(), 128, ref packing, ref offset, ref packed);
    pack_inner(@input.high.into(), 128, ref packing, ref offset, ref packed);
    packed.append(packing);

    assert(*packed.at(0) == 2000, 'Packing low value');
    assert(*packed.at(1) == 0, 'Packing high value');

    let mut unpacking: felt252 = packed.pop_front().unwrap();
    let mut un_offset = 0;
    let mut packed_span = packed.span();

    let low = unpack_inner(128, ref packed_span, ref unpacking, ref un_offset).unwrap();
    let high = unpack_inner(128, ref packed_span, ref unpacking, ref un_offset).unwrap();
    assert(
        u256 { low: low.try_into().unwrap(), high: high.try_into().unwrap() } == input,
        'Unpacked equals packed'
    );
}

#[test]
#[available_gas(9000000)]
fn test_pack_unpack_u256_single() {
    let input: u256 = 2000;
    let mut unpacked = ArrayTrait::new();
    input.serialize(ref unpacked);
    let mut layout = ArrayTrait::new();
    layout.append(128);
    layout.append(128);
    let mut layout_span = layout.span();

    let mut unpacked_span = unpacked.span();

    let mut packed = ArrayTrait::new();
    pack(ref packed, ref unpacked_span, 0, ref layout_span);

    let mut layout = ArrayTrait::new();
    layout.append(128);
    layout.append(128);
    let mut layout_span = layout.span();

    let mut unpacked = ArrayTrait::new();
    let mut packed_span = packed.span();
    unpack(ref unpacked, ref packed_span, ref layout_span);
    let mut unpacked_span = unpacked.span();
    let output = core::serde::Serde::<u256>::deserialize(ref unpacked_span).unwrap();
    assert(input == output, 'invalid output');
}

#[test]
#[available_gas(9000000)]
fn test_pack_unpack_max_felt252() {
    let MAX: felt252 = 3618502788666131213697322783095070105623107215331596699973092056135872020480;
    let mut packed = ArrayTrait::new();
    let mut packing: felt252 = 0;
    let mut offset = 0;
    pack_inner(@MAX, 251, ref packing, ref offset, ref packed);
    packed.append(packing);

    let mut unpacking: felt252 = 0;
    let mut offset = 251;
    let mut packed_span = packed.span();

    let got = unpack_inner(251, ref packed_span, ref unpacking, ref offset).unwrap();
    assert(got == MAX, 'Types MAX');
}

#[test]
#[available_gas(9000000)]
fn test_pack_unpack_felt252_single() {
    let input = 2000;
    let mut unpacked = ArrayTrait::new();
    input.serialize(ref unpacked);
    let mut layout = ArrayTrait::new();
    layout.append(251);
    let mut layout_span = layout.span();

    let mut unpacked_span = unpacked.span();

    let mut packed = ArrayTrait::new();
    pack(ref packed, ref unpacked_span, 0, ref layout_span);

    let mut layout = ArrayTrait::new();
    layout.append(251);
    let mut layout_span = layout.span();

    let mut unpacked = ArrayTrait::new();
    let mut packed_span = packed.span();
    unpack(ref unpacked, ref packed_span, ref layout_span);
    let mut unpacked_span = unpacked.span();
    let output = core::serde::Serde::<felt252>::deserialize(ref unpacked_span).unwrap();
    assert(input == output, 'invalid output');
}

#[test]
fn test_pack_with_offset() {
    let mut packed = array![];
    let mut unpacked = [1, 2, 3, 4, 5, 6, 7, 8, 9].span();
    let mut layout = [16, 128, 128, 8].span();

    pack(ref packed, ref unpacked, 5, ref layout);

    assert!(packed.len() == 2, "bad packed length");

    assert!(*packed.at(0) == 0x70006, "bad packed first item");
    assert!(*packed.at(1) == 0x0900000000000000000000000000000008, "bad packed second item");
}

#[test]
#[available_gas(9000000)]
fn test_calculate_packed_size() {
    let mut layout = [128, 32].span();
    let got = calculate_packed_size(ref layout);
    assert(got == 1, 'invalid length for [128, 32]');

    let mut layout = [128, 128].span();
    let got = calculate_packed_size(ref layout);
    assert(got == 2, 'invalid length for [128, 128]');

    let mut layout = [251, 251].span();
    let got = calculate_packed_size(ref layout);
    assert(got == 2, 'invalid length for [251, 251]');

    let mut layout = [251].span();
    let got = calculate_packed_size(ref layout);
    assert(got == 1, 'invalid length for [251]');

    let mut layout = [32, 64, 128, 27].span();
    let got = calculate_packed_size(ref layout);
    assert(got == 1, 'invalid length');

    let mut layout = [32, 64, 128, 28].span();
    let got = calculate_packed_size(ref layout);
    assert(got == 2, 'invalid length');
}

#[test]
#[available_gas(9000000)]
#[should_panic(expected: ('Invalid layout size',))]
fn test_pack_max_bits_value() {
    let mut unpacked = array![1];
    let mut layout = array![253];

    let mut layout_span = layout.span();
    let mut unpacked_span = unpacked.span();

    let mut packed = array![];
    pack(ref packed, ref unpacked_span, 0, ref layout_span);
}

#[test]
#[should_panic(expected: ('mismatched input lens',))]
fn test_pack_with_offset_exceeds_length() {
    let mut packed = array![];
    let mut unpacked = [1, 2, 3, 4, 5, 6, 7, 8, 9].span();
    let mut layout = [16, 128, 128, 8].span();

    pack(ref packed, ref unpacked, 6, ref layout);
}

#[test]
#[should_panic(expected: ('mismatched input lens',))]
fn test_pack_with_offset_layout_too_long() {
    let mut packed = array![];
    let mut unpacked = [1, 2, 3, 4, 5, 6, 7, 8, 9].span();
    let mut layout = [16, 128, 128, 8, 251].span();

    pack(ref packed, ref unpacked, 5, ref layout);
}

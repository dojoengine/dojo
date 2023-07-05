use array::{ArrayTrait, SpanTrait};
use starknet::{ClassHash, ContractAddress, Felt252TryIntoContractAddress, Felt252TryIntoClassHash};
use dojo::packable::{
    Packable, 
    PackableU8, 
    PackableU16, 
    PackableU32, 
    PackableU64, 
    PackableU128, 
    PackableFelt252,
    PackableBool, 
    shl, shr, fpow
};
use integer::U256BitAnd;
use option::OptionTrait;
use debug::PrintTrait;
use traits::{Into, TryInto};

#[test]
#[available_gas(9000000)]
fn test_bit_fpow() {
    assert(fpow(2, 250) == 1809251394333065553493296640760748560207343510400633813116524750123642650624_u256, '')
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
    let mut packed = array::ArrayTrait::new();
    let mut packing: felt252 = 0;
    let mut offset = 0;
    18_u8.pack(ref packing, ref offset, ref packed);
    packed.append(packing);

    assert(*packed.at(0) == 18_felt252, 'Packing single value');

    let mut unpacking: felt252 = packed.pop_front().unwrap();
    let mut un_offset = 0;
    let mut packed_span = packed.span();

    let result = Packable::<u8>::unpack(ref packed_span, ref unpacking, ref un_offset).unwrap();
    assert(result == 18_u8, 'Unpacked equals packed');
}

#[test]
#[available_gas(100000000)]
fn test_pack_multiple() {
    let mut packed = array::ArrayTrait::new();
    let mut packing: felt252 = 0;
    let mut offset = 0;

    let mut i: u32 = 0;
    loop {
        if i >= 20 {
            break;
        }
        i.pack(ref packing, ref offset, ref packed);
        i += 1;
    };    
    packed.append(packing);

    assert(
        *packed.at(0) == 188719626707717088982296698380167795313645871959412740063448560304128_felt252, 
        'Packed multiple 0'
    );
    assert(
        *packed.at(1) == 12940774403044448679501384905228180427841125450824163620418038790095104_felt252, 
        'Packed multiple 1'
    );
    assert(
        *packed.at(2) == 1541463130217537339061372343828480_felt252, 
        'Packed multiple 2'
    );
}

#[test]
#[available_gas(500000000)]
fn test_pack_unpack_multiple() {
    let mut packed = array::ArrayTrait::new();
    let mut packing: felt252 = 0;
    let mut offset = 0;

    let mut i: u8 = 0;
    loop {
        if i >= 40 {
            break;
        }
        let mut j: u32 = i.into();
        j = (j + 3) * j;

        i.pack(ref packing, ref offset, ref packed);
        j.pack(ref packing, ref offset, ref packed);

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
        let result_i = Packable::<u8>::unpack(ref packed_span, ref unpacking, ref un_offset).unwrap();
        let result_j = Packable::<u32>::unpack(ref packed_span, ref unpacking, ref un_offset).unwrap();

        let mut j: u32 = i.into();
        j = (j + 3) * j;

        assert(result_i == i, 'Unpacked equals packed');
        assert(result_j == j, 'Unpacked equals packed');
        i += 1;
    };
}

#[test]
#[available_gas(500000000)]
fn test_pack_unpack_types() {
    let mut packed = array::ArrayTrait::new();
    let mut packing: felt252 = 0;
    let mut offset = 0;

    let mut i: u8 = 0;
    3_u8.pack(ref packing, ref offset, ref packed);   
    14_u16.pack(ref packing, ref offset, ref packed);   
    59_u32.pack(ref packing, ref offset, ref packed);   
    26_u64.pack(ref packing, ref offset, ref packed);   
    53_u128.pack(ref packing, ref offset, ref packed);   
    58_felt252.pack(ref packing, ref offset, ref packed); 
    false.pack(ref packing, ref offset, ref packed);  
    
    let contract_address = Felt252TryIntoContractAddress::try_into(0).unwrap();
    contract_address.pack(ref packing, ref offset, ref packed);  
    let class_hash = Felt252TryIntoClassHash::try_into(0).unwrap();
    class_hash.pack(ref packing, ref offset, ref packed);  
    
    packed.append(packing);

    let mut unpacking: felt252 = packed.pop_front().unwrap();
    let mut un_offset = 0;
    let mut packed_span = packed.span();

    assert(
        Packable::<u8>::unpack(ref packed_span, ref unpacking, ref un_offset).unwrap() 
        == 3_u8, 
        'Types u8'
    );
    assert(
        Packable::<u16>::unpack(ref packed_span, ref unpacking, ref un_offset).unwrap() 
        == 14_u16, 
        'Types u16'
    );
    assert(
        Packable::<u32>::unpack(ref packed_span, ref unpacking, ref un_offset).unwrap() 
        == 59_u32, 
        'Types u32'
    );
    assert(
        Packable::<u64>::unpack(ref packed_span, ref unpacking, ref un_offset).unwrap() 
        == 26_u64, 
        'Types u64'
    );
    assert(
        Packable::<u128>::unpack(ref packed_span, ref unpacking, ref un_offset).unwrap() 
        == 53_u128, 
        'Types u128'
    );
    assert(
        Packable::<felt252>::unpack(ref packed_span, ref unpacking, ref un_offset).unwrap() 
        == 58_felt252, 
        'Types felt252'
    );
    assert(
        Packable::<bool>::unpack(ref packed_span, ref unpacking, ref un_offset).unwrap() 
        == false, 
        'Types bool'
    );
    assert(
        Packable::<ContractAddress>::unpack(ref packed_span, ref unpacking, ref un_offset).unwrap() 
        == contract_address, 
        'Types ContractAddress'
    );
    assert(
        Packable::<ClassHash>::unpack(ref packed_span, ref unpacking, ref un_offset).unwrap() 
        == class_hash, 
        'Types ClassHash'
    );
}

use array::{ArrayTrait, SpanTrait};
use dojo::packable::{Packable, PackableU8, PackableU32};
use option::OptionTrait;
use debug::PrintTrait;

#[test]
#[available_gas(2000000)]
fn test_basic_package() {
    let mut packed = array::ArrayTrait::new();
    let mut packing: felt252 = 0;
    let mut offset = 0;
    18_u8.pack(ref packing, ref offset, ref packed);
    let mut unpacking: felt252 = 0;
    let mut un_offset = 0;
    let mut packed_span = packed.span();

    let result = Packable::<u8>::unpack(ref packed_span, ref unpacking, ref un_offset).unwrap();
    result.print();
    assert(result == 18_u8, 'Expect equal');
}

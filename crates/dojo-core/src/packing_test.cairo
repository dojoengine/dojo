use array::{ArrayTrait, SpanTrait};
use starknet::{ClassHash, ContractAddress, Felt252TryIntoContractAddress, Felt252TryIntoClassHash};
use dojo::packing::{shl, shr, fpow};
use integer::U256BitAnd;
use option::OptionTrait;
use debug::PrintTrait;
use traits::{Into, TryInto};

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
#[available_gas(9000000)]
fn test_bit_shift() {
    assert(1 == shl(1, 0), 'left == right');
    assert(1 == shr(1, 0), 'left == right');

    assert(16 == shl(1, 4), 'left == right');
    assert(1 == shr(16, 4), 'left == right');

    assert(shr(shl(1, 251), 251) == 1, 'left == right')
}

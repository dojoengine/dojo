use array::{ArrayTrait, SpanTrait};
use option::OptionTrait;
use traits::Into;

use dojo_core::integer::u250;
use dojo_core::storage::utils::find_matching;

#[test]
#[available_gas(1000000000)]
fn test_find_matching() {
    let mut a1: Array<u250> = ArrayTrait::new();
    let mut a2: Array<u250> = ArrayTrait::new();
    let mut a3: Array<u250> = ArrayTrait::new();

    a1.append(1.into());
    a1.append(3.into());
    a1.append(6.into());
    a1.append(5.into());

    a2.append(4.into());
    a2.append(5.into());
    a2.append(3.into());

    a3.append(3.into());
    a3.append(2.into());
    a3.append(1.into());
    a3.append(7.into());
    a3.append(5.into());

    let (r1, r2, r3, r4) = find_matching(a1.span(), a2.span(), Option::Some(a3.span()), Option::None(()));
    assert(r1.len() == 2, 'r1 len');
    assert(r2.len() == 2, 'r2 len');
    let r3 = r3.expect('r3 is not Some');
    assert(r3.len() == 2, 'r3 len');
    assert(r4.is_none(), 'r4 is none');

    assert(*r1[0] == 1, 'r1[0]');
    assert(*r1[1] == 3, 'r1[1]');
    assert(*r2[0] == 2, 'r2[0]');
    assert(*r2[1] == 1, 'r2[1]');
    assert(*r3[0] == 0, 'r3[0]');
    assert(*r3[1] == 4, 'r3[1]');

    let mut a5: Array<u250> = ArrayTrait::new();
    let mut a6: Array<u250> = ArrayTrait::new();

    a5.append(10.into());
    a5.append(11.into());

    a6.append(12.into());
    a6.append(13.into());

    let (r5, r6, r7, r8) = find_matching(a5.span(), a6.span(), Option::None(()), Option::None(()));
    assert(r5.len() == 0, 'r5 len');
    assert(r6.len() == 0, 'r6 len');
}

#[test]
#[available_gas(1000000000)]
#[should_panic(expected: ('wrong argument order', ))]
fn test_find_matching_wrong_arg_order() {
    let mut a1: Array<u250> = ArrayTrait::new();
    let mut a2: Array<u250> = ArrayTrait::new();
    let mut a3: Array<u250> = ArrayTrait::new();

    a1.append(5.into());
    a2.append(5.into());
    a3.append(5.into());

    find_matching(a1.span(), a2.span(), Option::None(()), Option::Some(a3.span()));
}

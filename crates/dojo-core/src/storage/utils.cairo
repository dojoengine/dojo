use array::{ArrayTrait, SpanTrait};
use dict::Felt252DictTrait;
use option::OptionTrait;
use traits::TryInto;

use dojo_core::integer::u250;

const FIND_2: u8 = 2;
const FIND_3: u8 = 3;
const FIND_4: u8 = 4;

// finds elements with matching values across all input arrays
// the function accepts 2 to 4 arrays (spans) through which it loops and returns
// a corresponding amount of spans, each of the same length, each holding an index
// of the position in the input array where all the other arrays have the same value
//
// to illustrate, imagine these two:
// s1: [10, 20, 30, 40, 50, 60]
// s2: [60, 40, 20]
//
// when called with these two, the function will return:
// r1: [1, 3, 5] // indexes of 20, 40, 60 in s1, because those values are found in s2 as well
// r2: [2, 1, 0] // indexes of 20, 40, 60 respectively in s2
// r3: Option::None(())
// r4: Opiton::None(())
//
// the function perserves the order of the elements from s1
fn find_matching(
    mut s1: Span<u250>,
    mut s2: Span<u250>,
    mut s3: Option<Span<u250>>,
    mut s4: Option<Span<u250>>,
) -> (Span<usize>, Span<usize>, Option<Span<usize>>, Option<Span<usize>>) {
    let mut seen_ids: Felt252Dict<u8> = Felt252DictTrait::new();
    let mut present_in_all: u8 = FIND_2;

    // iterate through the second array and mark all its IDs
    let mut s2ii: Felt252Dict<usize> = Felt252DictTrait::new(); // mapping of ID to its index in s2
    let mut index: usize = 0;
    loop {
        match s2.pop_front() {
            Option::Some(id) => {
                seen_ids.insert(*id.inner, FIND_2);
                s2ii.insert(*id.inner, index);
                index += 1;
            },
            Option::None(_) => {
                break ();
            }
        };
    };

    // if we have a third array, iterate through it; if there's
    // an ID in s3 that has also been encountered in s2, mark it
    // and store the s3 value into a dict, using the matching ID as key
    let mut s3ii: Felt252Dict<usize> = Felt252DictTrait::new();
    if s3.is_some() {
        let mut s3 = s3.unwrap();
        let mut index: usize = 0;
        loop {
            match s3.pop_front() {
                Option::Some(id) => {
                    if seen_ids[*id.inner] == FIND_2 {
                        seen_ids.insert(*id.inner, FIND_3);
                        s3ii.insert(*id.inner, index);
                    }
                    index += 1;
                },
                Option::None(_) => {
                    break ();
                }
            };
        };
        present_in_all = FIND_3;
    }

    // similar as with s3, iterate through fourth array if present,
    // mark IDs that are found in both s2 and s3, mark those that
    // are in s4 and store their values in a dict
    let mut s4ii: Felt252Dict<usize> = Felt252DictTrait::new(); 
    if s4.is_some() {
        // preventing *not* passing in s3 but passing in s4
        //assert(got_s3, 'wrong argument order');
        assert(s3.is_some(), 'wrong argument order');

        let mut s4 = s4.unwrap();
        let mut index: usize = 0;
        loop {
            match s4.pop_front() {
                Option::Some(id) => {
                    if seen_ids[*id.inner] == FIND_3 {
                        seen_ids.insert(*id.inner, FIND_4);
                        s4ii.insert(*id.inner, index);
                    }
                    index += 1;
                },
                Option::None(_) => {
                    break ();
                }
            };
        };
        present_in_all = FIND_4;
    }

    // finally, loop through the first array (as last to keep its ID order),
    // and populate the return arrays
    let mut r1: Array<usize> = ArrayTrait::new();
    let mut r2: Array<usize> = ArrayTrait::new();
    let mut r3: Array<usize> = ArrayTrait::new();
    let mut r4: Array<usize> = ArrayTrait::new();

    index = 0;
    loop {
        match s1.pop_front() {
            Option::Some(id) => {
                let id = *id.inner;
                // if the current ID from a1 has been
                // seen in every zipped array
                if seen_ids[id] == present_in_all {
                    // add index from s1
                    r1.append(index);

                    // add index from s2
                    r2.append(s2ii.get(id));

                    // if we're zipping 3 arrays, add index from s3
                    let i3 = if (present_in_all >= FIND_3) {
                        r3.append(s3ii.get(id));
                    };

                    // if we're zipping 4 arrays, add index from s4
                    let i4 = if (present_in_all == FIND_4) {
                        r4.append(s4ii.get(id));
                    };
                }
                index += 1;
            },
            Option::None(_) => {
                break ();
            }
        };
    };

    seen_ids.squash();
    s2ii.squash();
    s3ii.squash();
    s4ii.squash();
    
    let or3: Option<Span<usize>> = {
        if s3.is_some() {
            Option::Some(r3.span())
        } else {
            Option::None(())
        }
    };
    let or4: Option<Span<usize>> = {
        if s4.is_some() {
            Option::Some(r4.span())
        } else {
            Option::None(())
        }
    };

    (r1.span(), r2.span(), or3, or4)
}

// #[test]
// #[available_gas(1000000000)]
// fn test_find_matching() {
//     let mut a1: Array<felt252> = ArrayTrait::new();
//     let mut a2: Array<felt252> = ArrayTrait::new();
//     let mut a3: Array<felt252> = ArrayTrait::new();

//     a1.append(1);
//     a1.append(3);
//     a1.append(6);
//     a1.append(5);

//     a2.append(4);
//     a2.append(5);
//     a2.append(3);

//     a3.append(3);
//     a3.append(2);
//     a3.append(1);
//     a3.append(7);
//     a3.append(5);

//     let (r1, r2, r3, r4) = find_matching(a1.span(), a2.span(), Option::Some(a3.span()), Option::None(()));
//     assert(r1.len() == 2, 'r1 len');
//     assert(r2.len() == 2, 'r2 len');
//     let r3 = r3.expect('r3 is not Some');
//     assert(r3.len() == 2, 'r3 len');
//     assert(r4.is_none(), 'r4 is none');

//     assert(*r1[0] == 1, 'r1[0]');
//     assert(*r1[1] == 3, 'r1[1]');
//     assert(*r2[0] == 2, 'r2[0]');
//     assert(*r2[1] == 1, 'r2[1]');
//     assert(*r3[0] == 0, 'r3[0]');
//     assert(*r3[1] == 4, 'r3[1]');

//     let mut a5: Array<felt252> = ArrayTrait::new();
//     let mut a6: Array<felt252> = ArrayTrait::new();

//     a5.append(10);
//     a5.append(11);
    
//     a6.append(12);
//     a6.append(13);

//     let (r5, r6, r7, r8) = find_matching(a5.span(), a6.span(), Option::None(()), Option::None(()));
//     assert(r5.len() == 0, 'r5 len');
//     assert(r6.len() == 0, 'r6 len');
// }

// #[test]
// #[available_gas(1000000000)]
// #[should_panic(expected: ('wrong argument order', ))]
// fn test_find_matching_wrong_arg_order() {
//     let mut a1: Array<felt252> = ArrayTrait::new();
//     let mut a2: Array<felt252> = ArrayTrait::new();
//     let mut a3: Array<felt252> = ArrayTrait::new();

//     a1.append(5);
//     a2.append(5);
//     a3.append(5);

//     find_matching(a1.span(), a2.span(), Option::None(()), Option::Some(a3.span()));
// }

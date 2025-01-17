use dojo::utils::{any_none, sum};

#[test]
fn test_any_none_when_one_none() {
    assert(
        any_none(@array![Option::Some(1_u8), Option::Some(2_u8), Option::None, Option::Some(3_u8)]),
        'None not found',
    )
}

#[test]
fn test_any_none_when_no_none() {
    assert(
        any_none(@array![Option::Some(1_u8), Option::Some(2_u8), Option::Some(3_u8)]) == false,
        'None found',
    )
}

#[test]
fn test_sum_when_empty_array() {
    assert(sum::<u8>(array![]) == 0, 'bad sum');
}

#[test]
fn test_sum_when_some_none_and_values() {
    assert(
        sum::<u8>(array![Option::Some(1), Option::None, Option::Some(2), Option::Some(3)]) == 6,
        'bad sum',
    );
}

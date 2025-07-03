use dojo::utils::sum_sizes;

#[test]
fn test_sum_sizes_when_one_none() {
    assert_eq!(sum_sizes(array![Some(1), Some(2), None, Some(3)]), None, "Bad sum_sizes with None");
}

#[test]
fn test_sum_sizes_when_no_none() {
    assert_eq!(
        sum_sizes(array![Some(1), Option::Some(2), Option::Some(3)]),
        Some(6),
        "Bad sum_sizes without None",
    );
}

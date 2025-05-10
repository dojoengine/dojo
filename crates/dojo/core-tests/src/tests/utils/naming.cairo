use dojo::utils::is_name_valid;

#[test]
fn test_with_valid_names() {
    assert!(is_name_valid(@"name"));
    assert!(is_name_valid(@"NAME"));
    assert!(is_name_valid(@"Name123"));
    assert!(is_name_valid(@"Name123_"));
}

#[test]
fn test_with_invalid_names() {
    assert!(!is_name_valid(@"n@me"));
    assert!(!is_name_valid(@"Name "));
    assert!(!is_name_valid(@"-name"));
}

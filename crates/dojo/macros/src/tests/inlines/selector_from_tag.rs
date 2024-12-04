use cairo_lang_macro::TokenStream;
use dojo_types::naming;

use crate::inlines::selector_from_tag::handle_selector_from_tag_macro;

#[test]
fn test_with_bad_type() {
    let input = TokenStream::new("enum MyEnum { X, Y }".to_string());

    let res = handle_selector_from_tag_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        "Macro `selector_from_tag!` must be called with a string parameter".to_string()
    );
}

#[test]
fn test_with_bad_argument_type() {
    let input = TokenStream::new("selector_from_tag![\"one\"]".to_string());

    let res = handle_selector_from_tag_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        "Macro `selector_from_tag!` does not support this bracket type.".to_string()
    );
}

#[test]
fn test_with_multiple_arguments() {
    let input = TokenStream::new("selector_from_tag!(\"one\", \"two\")".to_string());

    let res = handle_selector_from_tag_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        "Invalid arguments. Expected \"selector_from_tag!(\"tag\")\"".to_string()
    );
}

#[test]
fn test_with_bad_tag() {
    let input = TokenStream::new("selector_from_tag!(\"my_contract\")".to_string());

    let res = handle_selector_from_tag_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        "Invalid tag. Tag must be in the format of `namespace-name`.".to_string()
    );
}

#[test]
fn test_nominal_case() {
    let input = TokenStream::new("selector_from_tag!(\"dojo-my_contract\")".to_string());

    let res = handle_selector_from_tag_macro(input);

    assert!(res.diagnostics.is_empty());
    assert_eq!(
        res.token_stream.to_string(),
        format!("{:#64x}", naming::compute_selector_from_tag("dojo-my_contract"))
    );
}

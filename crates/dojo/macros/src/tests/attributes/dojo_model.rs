use cairo_lang_macro::TokenStream;

use crate::attributes::constants::{DOJO_EVENT_ATTR, DOJO_MODEL_ATTR};
use crate::attributes::dojo_model::handle_model_attribute_macro;
use crate::tests::utils::assert_output_stream;

const SIMPLE_MODEL: &str = "
#[derive(Introspect, Drop, Serde)]
struct SimpleModel {
    #[key]
    k: u32,
    v: u32
}";

const SIMPLE_MODEL_WITHOUT_INTROSPECT: &str = "
#[derive(Drop, Serde)]
struct SimpleModel {
    #[key]
    k: u32,
    v: u32
}";

const EXPANDED_SIMPLE_MODEL: &str = include_str!("./expanded/simple_model.cairo");

const COMPLEX_MODEL: &str = "
#[derive(Introspect, Drop, Serde)]
struct ComplexModel {
    #[key]
    k1: u8,
    #[key]
    k2: u32,
    v1: u256,
    v2: Option<u128>
}";

const EXPANDED_COMPLEX_MODEL: &str = include_str!("./expanded/complex_model.cairo");

#[test]
fn test_model_is_not_a_struct() {
    let input = TokenStream::new("enum MyEnum { X, Y }".to_string());

    let res = handle_model_attribute_macro(input);

    assert!(res.diagnostics.is_empty());
    assert!(res.token_stream.is_empty());
}

#[test]
fn test_model_has_duplicated_attributes() {
    let input = TokenStream::new(format!(
        "
        #[{DOJO_MODEL_ATTR}]
        {SIMPLE_MODEL}
        "
    ));

    let res = handle_model_attribute_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        format!("Only one {DOJO_MODEL_ATTR} attribute is allowed per module.")
    );
}

#[test]
fn test_model_has_attribute_conflict() {
    let input = TokenStream::new(format!(
        "
        #[{DOJO_EVENT_ATTR}]
        {SIMPLE_MODEL}
        "
    ));

    let res = handle_model_attribute_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        format!("A {DOJO_MODEL_ATTR} can't be used together with a {DOJO_EVENT_ATTR}.")
    );
}

#[test]
fn test_model_has_no_key() {
    let input = TokenStream::new(
        "
        #[derive(Introspect, Drop, Serde)]
        struct ModelNoKey {
            k: u32,
            v: u32
        }
        "
        .to_string(),
    );

    let res = handle_model_attribute_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        "Model must define at least one #[key] attribute".to_string()
    );
}

#[test]
fn test_model_has_no_value() {
    let input = TokenStream::new(
        "
        #[derive(Introspect, Drop, Serde)]
        struct ModelNoValue {
            #[key]
            k: u32
        }
        "
        .to_string(),
    );

    let res = handle_model_attribute_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        "Model must define at least one member that is not a key".to_string()
    );
}

#[test]
fn test_model_derives_from_both_introspect_and_packed() {
    let input = TokenStream::new(
        "
        #[derive(Introspect, IntrospectPacked, Drop, Serde)]
        struct SimpleModel {
            #[key]
            k: u32,
            v: u32
        }
        "
        .to_string(),
    );

    let res = handle_model_attribute_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        "Model cannot derive from both Introspect and IntrospectPacked.".to_string()
    );
}

#[test]
fn test_model_does_not_derive_from_drop() {
    let input = TokenStream::new(
        "
        #[derive(Serde)]
        struct SimpleModel {
            #[key]
            k: u32,
            v: u32
        }
        "
        .to_string(),
    );

    let res = handle_model_attribute_macro(input);

    assert_eq!(res.diagnostics[0].message, "Model must derive from Drop and Serde.".to_string());
}

#[test]
fn test_model_does_not_derive_from_serde() {
    let input = TokenStream::new(
        "
        #[derive(Drop)]
        struct SimpleModel {
            #[key]
            k: u32,
            v: u32
        }
        "
        .to_string(),
    );

    let res = handle_model_attribute_macro(input);

    assert_eq!(res.diagnostics[0].message, "Model must derive from Drop and Serde.".to_string());
}

#[test]
fn test_simple_model() {
    let input = TokenStream::new(SIMPLE_MODEL.to_string());

    let res = handle_model_attribute_macro(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_SIMPLE_MODEL);
}

#[test]
fn test_simple_model_without_introspect() {
    let input = TokenStream::new(SIMPLE_MODEL_WITHOUT_INTROSPECT.to_string());

    let res = handle_model_attribute_macro(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_SIMPLE_MODEL);
}

#[test]
fn test_complex_model() {
    let input = TokenStream::new(COMPLEX_MODEL.to_string());

    let res = handle_model_attribute_macro(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_COMPLEX_MODEL);
}

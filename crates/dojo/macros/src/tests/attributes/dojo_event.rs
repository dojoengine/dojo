use cairo_lang_macro::TokenStream;

use crate::attributes::constants::{DOJO_EVENT_ATTR, DOJO_MODEL_ATTR};
use crate::attributes::dojo_event::handle_event_attribute_macro;
use crate::derives::DOJO_PACKED_DERIVE;
use crate::tests::utils::assert_output_stream;

const SIMPLE_EVENT_WITHOUT_INTROSPECT: &str = "
#[derive(Drop, Serde)]
struct SimpleEvent {
    #[key]
    k: u32,
    v: u32
}";

const SIMPLE_EVENT: &str = "
#[derive(Introspect, Drop, Serde)]
struct SimpleEvent {
    #[key]
    k: u32,
    v: u32
}";

const EXPANDED_SIMPLE_EVENT: &str = include_str!("./expanded/simple_event.cairo");

const COMPLEX_EVENT: &str = "
#[derive(Introspect, Drop, Serde)]
struct ComplexEvent {
    #[key]
    k1: u8,
    #[key]
    k2: u32,
    v1: u256,
    v2: Option<u128>
}";

const EXPANDED_COMPLEX_EVENT: &str = include_str!("./expanded/complex_event.cairo");

#[test]
fn test_event_is_not_a_struct() {
    let input = TokenStream::new("enum MyEnum { X, Y }".to_string());

    let res = handle_event_attribute_macro(input);

    assert!(res.diagnostics.is_empty());
    assert!(res.token_stream.is_empty());
}

#[test]
fn test_event_has_duplicated_attributes() {
    let input = TokenStream::new(format!(
        "
        #[{DOJO_EVENT_ATTR}]
        {SIMPLE_EVENT}
        "
    ));

    let res = handle_event_attribute_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        format!("Only one {DOJO_EVENT_ATTR} attribute is allowed per module.")
    );
}

#[test]
fn test_event_has_attribute_conflict() {
    let input = TokenStream::new(format!(
        "
        #[{DOJO_MODEL_ATTR}]
        {SIMPLE_EVENT}
        "
    ));

    let res = handle_event_attribute_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        format!("A {DOJO_EVENT_ATTR} can't be used together with a {DOJO_MODEL_ATTR}.")
    );
}

#[test]
fn test_event_has_no_key() {
    let input = TokenStream::new(
        "
        #[derive(Introspect, Drop, Serde)]
        struct EventNoKey {
            k: u32,
            v: u32
        }
        "
        .to_string(),
    );

    let res = handle_event_attribute_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        "Event must define at least one #[key] attribute".to_string()
    );
}

#[test]
fn test_event_has_no_value() {
    let input = TokenStream::new(
        "
        #[derive(Introspect, Drop, Serde)]
        struct EventNoValue {
            #[key]
            k: u32
        }
        "
        .to_string(),
    );

    let res = handle_event_attribute_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        "Event must define at least one member that is not a key".to_string()
    );
}

#[test]
fn test_event_derives_from_introspect_packed() {
    let input = TokenStream::new(
        "
        #[derive(IntrospectPacked, Drop, Serde)]
        struct SimpleEvent {
            #[key]
            k: u32,
            v: u32
        }
        "
        .to_string(),
    );

    let res = handle_event_attribute_macro(input);

    assert_eq!(
        res.diagnostics[0].message,
        format!("Deriving {DOJO_PACKED_DERIVE} on event is not allowed.")
    );
}

#[test]
fn test_event_does_not_derive_from_drop() {
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

    let res = handle_event_attribute_macro(input);

    assert_eq!(res.diagnostics[0].message, "Event must derive from Drop and Serde.".to_string());
}

#[test]
fn test_event_does_not_derive_from_serde() {
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

    let res = handle_event_attribute_macro(input);

    assert_eq!(res.diagnostics[0].message, "Event must derive from Drop and Serde.".to_string());
}

#[test]
fn test_simple_event_without_introspect() {
    let input = TokenStream::new(SIMPLE_EVENT_WITHOUT_INTROSPECT.to_string());

    let res = handle_event_attribute_macro(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_SIMPLE_EVENT);
}

#[test]
fn test_simple_event() {
    let input = TokenStream::new(SIMPLE_EVENT.to_string());

    let res = handle_event_attribute_macro(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_SIMPLE_EVENT);
}

#[test]
fn test_complex_event() {
    let input = TokenStream::new(COMPLEX_EVENT.to_string());

    let res = handle_event_attribute_macro(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_COMPLEX_EVENT);
}

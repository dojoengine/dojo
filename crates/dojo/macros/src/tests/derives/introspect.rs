use cairo_lang_macro::TokenStream;

use crate::derives::handle_derives_macros;
use crate::tests::utils::assert_output_stream;

const SIMPLE_STRUCT: &str = "
#[derive(Introspect)]
struct SimpleStruct {
    #[key]
    k1: u256,
    v1: u32,
    v2: (u8, u16),
}
";

const EXPANDED_SIMPLE_STRUCT: &str = include_str!("./expanded/simple_struct.cairo");

const PACKED_STRUCT: &str = "
#[derive(IntrospectPacked)]
struct SimpleStruct {
    #[key]
    k1: u256,
    v1: u32,
    v2: (u8, u16),
}
";

const EXPANDED_PACKED_STRUCT: &str = include_str!("./expanded/packed_struct.cairo");

const COMPLEX_STRUCT: &str = "
#[derive(Introspect)]
struct ComplexStruct {
    #[key]
    k1: u256,
    #[key]
    k2: u32,
    v1: Array<u32>,
    v2: Option<u128>,
    v3: (Array<u8>, u16, Option<u64>)
}
";

const EXPANDED_COMPLEX_STRUCT: &str = include_str!("./expanded/complex_struct.cairo");

const SIMPLE_ENUM: &str = "
#[derive(Introspect)]
enum SimpleEnum {
    VARIANT1,
    VARIANT2,
    VARIANT3
}
";

const EXPANDED_SIMPLE_ENUM: &str = include_str!("./expanded/simple_enum.cairo");

const PACKED_ENUM: &str = "
#[derive(Introspect)]
enum PackedEnum {
    VARIANT1: (u32, u128),
    VARIANT2: (u32, u128),
    VARIANT3: (u32, u128),
}
";

const EXPANDED_PACKED_ENUM: &str = include_str!("./expanded/packed_enum.cairo");

const COMPLEX_ENUM: &str = "
#[derive(Introspect)]
enum ComplexEnum {
    VARIANT1: u32,
    VARIANT2: Option<u64>,
    VARIANT3: (u8, u16, u32)
}
";

const EXPANDED_COMPLEX_ENUM: &str = include_str!("./expanded/complex_enum.cairo");

#[test]
fn test_bad_type() {
    let input = TokenStream::new("mod my_module {}".to_string());

    let res = handle_derives_macros(input);

    assert!(res.diagnostics.is_empty());
    assert!(res.token_stream.is_empty());
}

#[test]
fn test_attribute_conflict() {
    let input = TokenStream::new(
        "#[derive(Introspect, IntrospectPacked)]
        struct MyStruct {
            v: u32
        }"
        .to_string(),
    );

    let res = handle_derives_macros(input);

    assert_eq!(
        res.diagnostics[0].message,
        format!("Introspect and IntrospectPacked attributes cannot be used at a same time.")
    );
}

#[test]
fn test_tuple_in_option_error() {
    let input = TokenStream::new(
        "#[derive(Introspect)]
        enum MyEnum {
            V1: Option<(u8, u32)>
            V2: Option<(u8, u32)>
        }"
        .to_string(),
    );

    let res = handle_derives_macros(input);

    assert_eq!(
        res.diagnostics[0].message,
        format!("Option<T> cannot be used with tuples. Prefer using a struct.")
    );
}

#[test]
fn test_bad_enum_for_introspect_packed() {
    let input = TokenStream::new(
        "#[derive(IntrospectPacked)]
        enum MyEnum {
            V1: Option<u32>,
            V2: u128
        }"
        .to_string(),
    );

    let res = handle_derives_macros(input);

    assert_eq!(
        res.diagnostics[0].message,
        format!("To be packed, all variants must have fixed layout of same size.")
    );
}

#[test]
fn test_simple_struct() {
    let input = TokenStream::new(SIMPLE_STRUCT.to_string());

    let res = handle_derives_macros(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_SIMPLE_STRUCT);
}

#[test]
fn test_packed_struct() {
    let input = TokenStream::new(PACKED_STRUCT.to_string());

    let res = handle_derives_macros(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_PACKED_STRUCT);
}

#[test]
fn test_complex_struct() {
    let input = TokenStream::new(COMPLEX_STRUCT.to_string());

    let res = handle_derives_macros(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_COMPLEX_STRUCT);
}

#[test]
fn test_simple_enum() {
    let input = TokenStream::new(SIMPLE_ENUM.to_string());

    let res = handle_derives_macros(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_SIMPLE_ENUM);
}

#[test]
fn test_packed_enum() {
    let input = TokenStream::new(PACKED_ENUM.to_string());

    let res = handle_derives_macros(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_PACKED_ENUM);
}

#[test]
fn test_complex_enum() {
    let input = TokenStream::new(COMPLEX_ENUM.to_string());

    let res = handle_derives_macros(input);

    assert!(res.diagnostics.is_empty());
    assert_output_stream(&res.token_stream, EXPANDED_COMPLEX_ENUM);
}

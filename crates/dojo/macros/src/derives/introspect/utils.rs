const TUPLE_PREFIX: &str = "(";
const TUPLE_SUFFIX: &str = ")";
const SPAN_PREFIX: &str = "Span<";
const SPAN_SUFFIX: &str = ">";
const ARRAY_PREFIX: &str = "Array<";
const ARRAY_SUFFIX: &str = ">";
const FIXED_ARRAY_PREFIX: &str = "[";

pub fn is_byte_array(ty: &str) -> bool {
    ty.eq("ByteArray")
}

pub fn is_array(ty: &str) -> bool {
    ty.starts_with(ARRAY_PREFIX) || ty.starts_with(SPAN_PREFIX)
}

pub fn is_tuple(ty: &str) -> bool {
    ty.starts_with(TUPLE_PREFIX)
}

pub fn is_fixed_size_array(ty: &str) -> bool {
    ty.starts_with(FIXED_ARRAY_PREFIX)
}

pub fn is_option(ty: &str) -> bool {
    ty.starts_with("Option<")
}

pub fn get_array_item_type(ty: &str) -> String {
    if ty.starts_with(ARRAY_PREFIX) {
        extract_composite_inner_type(ty, ARRAY_PREFIX, ARRAY_SUFFIX)
    } else {
        extract_composite_inner_type(ty, SPAN_PREFIX, SPAN_SUFFIX)
    }
}

pub fn extract_fixed_array_type(ty: &str) -> (String, u32) {
    let re = regex::Regex::new(r"\[(.*);\s*(\d+)\]").unwrap();
    let caps = re.captures(ty).unwrap_or_else(|| {
        panic!("'{ty}' must follow the fixed size array format '[TYPE; SIZE]'.")
    });
    (caps[1].to_string(), caps[2].parse::<u32>().unwrap())
}

/// Extracts the inner type of a composite type such as tuple, array or span.
///
/// # Arguments
///   * `ty` - the composite type
///   * `prefix` - the prefix used to delimit the beginning of the composite type
///   * `suffix` - the suffix used to delimit the end of the composite type
///
/// # Examples
///    extract_composite_inner_type("Array<(u8, u16)", "Array<", ">") returns "u8, u16"
pub fn extract_composite_inner_type(ty: &str, prefix: &str, suffix: &str) -> String {
    let re = regex::Regex::new(&format!("{}(.*){}", regex::escape(prefix), regex::escape(suffix)))
        .unwrap();

    let caps = re.captures(ty).unwrap_or_else(|| {
        panic!("'{ty}' must contain the '{prefix}' prefix and the '{suffix}' suffix.")
    });

    caps[1].to_string().replace(" ", "")
}

/// split a tuple in array of items (nested tuples are not split).
/// example (u8, (u16, u32), u128) -> ["u8", "(u16, u32)", "u128"]
pub fn get_tuple_item_types(ty: &str) -> Vec<String> {
    let tuple_str = extract_composite_inner_type(ty, TUPLE_PREFIX, TUPLE_SUFFIX);
    let mut items = vec![];
    let mut current_item = "".to_string();
    let mut level = 0;

    for c in tuple_str.chars() {
        if c == ',' {
            if level > 0 {
                current_item.push(c);
            }

            if level == 0 && !current_item.is_empty() {
                items.push(current_item);
                current_item = "".to_string();
            }
        } else {
            current_item.push(c);

            if c.to_string() == TUPLE_PREFIX {
                level += 1;
            }
            if c.to_string() == TUPLE_SUFFIX {
                level -= 1;
            }
        }
    }

    if !current_item.is_empty() {
        items.push(current_item);
    }

    items
}

#[test]
pub fn test_get_tuple_item_types() {
    pub fn assert_array(got: Vec<String>, expected: Vec<String>) {
        pub fn format_array(arr: Vec<String>) -> String {
            format!("[{}]", arr.join(", "))
        }

        assert!(
            got.len() == expected.len(),
            "arrays have not the same length (got: {}, expected: {})",
            format_array(got),
            format_array(expected)
        );

        for i in 0..got.len() {
            assert!(
                got[i] == expected[i],
                "unexpected array item: (got: {} expected: {})",
                got[i],
                expected[i]
            )
        }
    }

    let test_cases = vec![
        ("()", vec![]),
        ("(u8,)", vec!["u8"]),
        ("(u8, u16, u32)", vec!["u8", "u16", "u32"]),
        ("(u8, (u16,), u32)", vec!["u8", "(u16,)", "u32"]),
        ("(u8, (u16, (u8, u16)))", vec!["u8", "(u16,(u8,u16))"]),
        ("(Array<(Points, Damage)>, ((u16,),)))", vec!["Array<(Points,Damage)>", "((u16,),))"]),
        (
            "(u8, (u16, (u8, u16), Array<(Points, Damage)>), ((u16,),)))",
            vec!["u8", "(u16,(u8,u16),Array<(Points,Damage)>)", "((u16,),))"],
        ),
    ];

    for (value, expected) in test_cases {
        assert_array(
            get_tuple_item_types(value),
            expected.iter().map(|x| x.to_string()).collect::<Vec<_>>(),
        )
    }
}

#[test]
fn test_extract_composite_inner_type_with_tuples() {
    let test_cases = [
        ("(u8,)", "u8,"),
        ("(u8,),", "u8,"),
        ("(u8, u16)", "u8,u16"),
        ("(u8, u16,)", "u8,u16,"),
        ("(u8, u16, (u32,))", "u8,u16,(u32,)"),
        ("(u8, u16, (u32,),)", "u8,u16,(u32,),"),
        (
            "(u8, (Span<u32>, u32, Option<Array<u8>,) u16, (u32,),)",
            "u8,(Span<u32>,u32,Option<Array<u8>,)u16,(u32,),",
        ),
        // Unity type is a special case (empty tuple).
        ("()", ""),
        ("(),", ""),
        ("((), (u8, ()))", "(),(u8,())"),
    ];

    for (tuple, expected) in test_cases {
        let result = extract_composite_inner_type(tuple, TUPLE_PREFIX, TUPLE_SUFFIX);
        assert!(
            result == expected,
            "bad tuple: {} result: {} expected: {}",
            tuple,
            result,
            expected
        );
    }
}

#[test]
#[should_panic(expected = "'u8, u16' must contain the '(' prefix and the ')' suffix.")]
fn test_extract_composite_inner_type_with_tuples_bad_ty() {
    let _ = extract_composite_inner_type("u8, u16", TUPLE_PREFIX, TUPLE_SUFFIX);
}

#[test]
fn test_extract_composite_inner_type_with_arrays() {
    let test_cases = [
        ("Array<u8>", "u8"),
        ("Array<(u8, u16)>", "(u8,u16)"),
        ("Array<Array<(u8, u16)>>", "Array<(u8,u16)>"),
        ("Array<(u8, u16)> // comment", "(u8,u16)"),
        ("Array<(u8, u16)>, // comment", "(u8,u16)"),
        // Array<()> is not supported by Cairo so no need to test this case
    ];

    for (arr, expected) in test_cases {
        let result = extract_composite_inner_type(arr, ARRAY_PREFIX, ARRAY_SUFFIX);
        assert!(result == expected, "bad array: {} result: {} expected: {}", arr, result, expected);
    }
}

#[test]
#[should_panic(expected = "'u8, u16' must contain the 'Array<' prefix and the '>' suffix.")]
fn test_extract_composite_inner_type_with_arrays_bad_ty() {
    let _ = extract_composite_inner_type("u8, u16", ARRAY_PREFIX, ARRAY_SUFFIX);
}

#[test]
fn test_extract_composite_inner_type_with_spans() {
    let test_cases = [
        ("Span<u8>", "u8"),
        ("Span<(u8, u16)>", "(u8,u16)"),
        ("Span<Array<(u8, u16)>>", "Array<(u8,u16)>"),
        ("Span<(u8, u16)> // comment", "(u8,u16)"),
        ("Span<(u8, u16)>, // comment", "(u8,u16)"),
        ("Span<()>", "()"),
    ];

    for (sp, expected) in test_cases {
        let result = extract_composite_inner_type(sp, SPAN_PREFIX, SPAN_SUFFIX);
        assert!(result == expected, "bad span: {} result: {} expected: {}", sp, result, expected);
    }
}

#[test]
#[should_panic(expected = "'u8, u16' must contain the 'Span<' prefix and the '>' suffix.")]
fn test_extract_composite_inner_type_with_spans_bad_ty() {
    let _ = extract_composite_inner_type("u8, u16", SPAN_PREFIX, SPAN_SUFFIX);
}

#[test]
fn test_extract_fixed_array_type() {
    let test_cases = [
        ("[u8;3]", ("u8", 3)),
        ("[u8; 3]", ("u8", 3)),
        ("[(u8, u16); 1]", ("(u8, u16)", 1)),
        ("[[u16; 1]; 2]", ("[u16; 1]", 2)),
        ("[[MyStruct; 1]; 2]", ("[MyStruct; 1]", 2)),
        ("[[(); 1]; 2]", ("[(); 1]", 2)),
    ];

    for (ty, (expected_type, expected_size)) in test_cases {
        let (item_type, size) = extract_fixed_array_type(ty);
        assert!(
            item_type == expected_type && size == expected_size,
            "bad fixed array: {} result: ({}, {}) expected: ({}, {})",
            ty,
            item_type,
            size,
            expected_type,
            expected_size
        );
    }
}

#[test]
#[should_panic(expected = "'u8;3]' must follow the fixed size array format '[TYPE; SIZE]'.")]
fn test_extract_fixed_array_type_missing_prefix() {
    let _ = extract_fixed_array_type("u8;3]");
}

#[test]
#[should_panic(expected = "'[u8;3' must follow the fixed size array format '[TYPE; SIZE]'.")]
fn test_extract_fixed_array_type_missing_suffix() {
    let _ = extract_fixed_array_type("[u8;3");
}

#[test]
#[should_panic(expected = "'[u8,3]' must follow the fixed size array format '[TYPE; SIZE]'.")]
fn test_extract_fixed_array_type_bad_separator() {
    let _ = extract_fixed_array_type("[u8,3]");
}

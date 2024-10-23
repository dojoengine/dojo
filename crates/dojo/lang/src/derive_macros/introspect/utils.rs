use std::collections::HashMap;

#[derive(Clone, Default, Debug)]
pub struct TypeIntrospection(pub usize, pub Vec<usize>);

// Provides type introspection information for primitive types
pub fn primitive_type_introspection() -> HashMap<String, TypeIntrospection> {
    HashMap::from([
        ("felt252".into(), TypeIntrospection(1, vec![251])),
        ("bool".into(), TypeIntrospection(1, vec![1])),
        ("u8".into(), TypeIntrospection(1, vec![8])),
        ("u16".into(), TypeIntrospection(1, vec![16])),
        ("u32".into(), TypeIntrospection(1, vec![32])),
        ("u64".into(), TypeIntrospection(1, vec![64])),
        ("u128".into(), TypeIntrospection(1, vec![128])),
        ("u256".into(), TypeIntrospection(2, vec![128, 128])),
        ("usize".into(), TypeIntrospection(1, vec![32])),
        ("ContractAddress".into(), TypeIntrospection(1, vec![251])),
        ("ClassHash".into(), TypeIntrospection(1, vec![251])),
    ])
}

/// Check if the provided type is an unsupported `Option<T>`,
/// because tuples are not supported with Option.
pub fn is_unsupported_option_type(ty: &str) -> bool {
    ty.starts_with("Option<(")
}

pub fn is_byte_array(ty: &str) -> bool {
    ty.eq("ByteArray")
}

pub fn is_array(ty: &str) -> bool {
    ty.starts_with("Array<") || ty.starts_with("Span<")
}

pub fn is_tuple(ty: &str) -> bool {
    ty.starts_with('(')
}

pub fn get_array_item_type(ty: &str) -> String {
    if ty.starts_with("Array<") {
        ty.trim()
            .strip_prefix("Array<")
            .unwrap()
            .strip_suffix('>')
            .unwrap()
            .to_string()
    } else {
        ty.trim()
            .strip_prefix("Span<")
            .unwrap()
            .strip_suffix('>')
            .unwrap()
            .to_string()
    }
}

/// split a tuple in array of items (nested tuples are not splitted).
/// example (u8, (u16, u32), u128) -> ["u8", "(u16, u32)", "u128"]
pub fn get_tuple_item_types(ty: &str) -> Vec<String> {
    let tuple_str = ty
        .trim()
        .strip_prefix('(')
        .unwrap()
        .strip_suffix(')')
        .unwrap()
        .to_string()
        .replace(' ', "");
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

            if c == '(' {
                level += 1;
            }
            if c == ')' {
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
        ("(u8,)", vec!["u8"]),
        ("(u8, u16, u32)", vec!["u8", "u16", "u32"]),
        ("(u8, (u16,), u32)", vec!["u8", "(u16,)", "u32"]),
        ("(u8, (u16, (u8, u16)))", vec!["u8", "(u16,(u8,u16))"]),
        (
            "(Array<(Points, Damage)>, ((u16,),)))",
            vec!["Array<(Points,Damage)>", "((u16,),))"],
        ),
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

use cairo_lang_syntax::node::ast::{Expr, TypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::TypedSyntaxNode;

use super::utils::{get_tuple_item_types, is_array, is_byte_array, is_tuple};

pub fn build_size_function_body(
    sizes: &mut Vec<String>,
    cumulated_sizes: u32,
    is_dynamic_size: bool,
    is_packed: bool,
) -> String {
    if is_dynamic_size {
        return "None".to_string();
    }

    if cumulated_sizes > 0 {
        sizes.push(format!("Some({})", cumulated_sizes));
    }

    match sizes.len() {
        0 => "None".to_string(),
        1 => sizes[0].clone(),
        _ => {
            // TODO RBA: use sum_sizes() from the metaprogramming PR
            let size_items = sizes.join(",");

            let none_check = if is_packed {
                "".to_string()
            } else {
                format!(
                    "
                    let mut it = array![{size_items}].into_iter();
                    if it.any(|x| x.is_none()) {{
                        return None;
                    }}"
                )
            };

            format!(
                "{none_check}
                let mut it = array![{size_items}].into_iter();
                Some(it.fold(0, |acc, x| acc + x.unwrap()))
                ",
            )
        }
    }
}

pub fn get_field_size_from_type_clause(
    db: &dyn SyntaxGroup,
    type_clause: &TypeClause,
) -> (Vec<String>, u32, bool) {
    let mut cumulated_sizes = 0;
    let mut is_dynamic_size = false;

    let field_sizes = match type_clause.ty(db) {
        Expr::Path(path) => {
            let path_type = path.as_syntax_node().get_text_without_trivia(db);
            compute_item_size_from_type(&path_type)
        }
        Expr::Tuple(expr) => {
            let tuple_type = expr.as_syntax_node().get_text_without_trivia(db);
            compute_tuple_size_from_type(&tuple_type)
        }
        _ => {
            // field type already checked while building the layout
            vec!["ERROR".to_string()]
        }
    };

    let sizes = field_sizes
        .into_iter()
        .filter_map(|s| match s.parse::<u32>() {
            Ok(v) => {
                cumulated_sizes += v;
                None
            }
            Err(_) => {
                if s.eq("None") {
                    is_dynamic_size = true;
                    None
                } else {
                    Some(s)
                }
            }
        })
        .collect::<Vec<_>>();

    (sizes, cumulated_sizes, is_dynamic_size)
}

pub fn compute_item_size_from_type(item_type: &String) -> Vec<String> {
    if is_array(item_type) || is_byte_array(item_type) {
        vec!["None".to_string()]
    } else if is_tuple(item_type) {
        compute_tuple_size_from_type(item_type)
    } else {
        vec![format!("dojo::meta::introspect::Introspect::<{}>::size()", item_type)]
    }
}

pub fn compute_tuple_size_from_type(tuple_type: &str) -> Vec<String> {
    get_tuple_item_types(tuple_type)
        .iter()
        .flat_map(compute_item_size_from_type)
        .collect::<Vec<_>>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_result(output: &str, expected: &str) {
        let output = output.replace(" ", "").replace("\n", "");
        let expected = expected.replace(" ", "").replace("\n", "");
        assert_eq!(output, expected);
    }

    #[test]
    fn test_build_size_function_body_when_dynamic_size() {
        let mut sizes = vec!["Some(1)".to_string()];
        let result = build_size_function_body(&mut sizes, 0, true, false);
        assert_result(&result, "None");
    }

    #[test]
    fn test_build_size_function_body_when_packed() {
        let mut sizes = vec!["Some(1)".to_string(), "Some(2)".to_string()];
        let result = build_size_function_body(&mut sizes, 3, false, true);
        assert_result(
            &result,
            "let mut it = array![Some(1), Some(2), Some(3)].into_iter();
            Some(it.fold(0, |acc, x| acc + x.unwrap()))",
        );
    }

    #[test]
    fn test_build_size_function_body_with_multiple_sizes_and_with_none() {
        let mut sizes = vec!["Some(1)".to_string(), "Some(2)".to_string(), "None".to_string()];
        let result = build_size_function_body(&mut sizes, 3, false, false);
        assert_result(
            &result,
            "
            let mut it = array![Some(1), Some(2), None, Some(3)].into_iter();
            if it.any(|x| x.is_none()) {
                return None;
            }
            let mut it = array![Some(1), Some(2), None, Some(3)].into_iter();
            Some(it.fold(0, |acc, x| acc + x.unwrap()))
            ",
        );
    }

    #[test]
    fn test_build_size_function_body_with_multiple_sizes_and_without_none() {
        let mut sizes = vec!["Some(1)".to_string(), "Some(2)".to_string()];
        let result = build_size_function_body(&mut sizes, 3, false, false);
        assert_result(
            &result,
            "let mut it = array![Some(1), Some(2), Some(3)].into_iter();
            if it.any(|x| x.is_none()) {
                return None;
            }
            let mut it = array![Some(1), Some(2), Some(3)].into_iter();
            Some(it.fold(0, |acc, x| acc + x.unwrap()))
            ",
        );
    }
}

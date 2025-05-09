use cairo_lang_syntax::node::TypedSyntaxNode;
use cairo_lang_syntax::node::ast::{Expr, TypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;

use super::utils::{get_tuple_item_types, is_array, is_byte_array, is_tuple};

pub fn build_size_function_body(
    sizes: &mut Vec<String>,
    cumulated_sizes: u32,
    is_dynamic_size: bool,
    is_packed: bool,
) -> String {
    if is_dynamic_size {
        return "Option::None".to_string();
    }

    if cumulated_sizes > 0 {
        sizes.push(format!("Option::Some({})", cumulated_sizes));
    }

    match sizes.len() {
        0 => "Option::None".to_string(),
        1 => sizes[0].clone(),
        _ => {
            let none_check = if is_packed {
                ""
            } else {
                "if dojo::utils::any_none(@sizes) {
                    return Option::None;
                }"
            };

            format!(
                "let sizes : Array<Option<usize>> = array![
                    {}
                ];

                {none_check}
                Option::Some(dojo::utils::sum(sizes))
                ",
                sizes.join(",\n")
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
                if s.eq("Option::None") {
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
        vec!["Option::None".to_string()]
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

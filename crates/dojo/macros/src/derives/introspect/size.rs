use cairo_lang_syntax::node::TypedSyntaxNode;
use cairo_lang_syntax::node::ast::{Expr, TypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;

use super::utils::{is_array, is_byte_array};

pub fn build_size_function_body(sizes: &mut [String]) -> String {
    match sizes.len() {
        0 => "Option::None".to_string(),
        1 => sizes[0].clone(),
        _ => {
            format!(
                "let sizes : Array<Option<usize>> = array![
                    {}
                ];

                if dojo::utils::any_none(@sizes) {{
                    return Option::None;
                }}

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
) -> Vec<String> {
    match type_clause.ty(db) {
        Expr::Path(path) => {
            let path_type = path.as_syntax_node().get_text_without_trivia(db);
            compute_item_size_from_type(&path_type)
        }
        Expr::Tuple(expr) => {
            if expr.expressions(db).elements(db).is_empty() {
                vec![]
            } else {
                let tuple_type = expr.as_syntax_node().get_text_without_trivia(db);
                compute_item_size_from_type(&tuple_type)
            }
        }
        _ => {
            // field type already checked while building the layout
            vec!["ERROR".to_string()]
        }
    }
}

pub fn compute_item_size_from_type(item_type: &String) -> Vec<String> {
    if is_array(item_type) || is_byte_array(item_type) {
        vec!["Option::None".to_string()]
    } else {
        vec![format!("dojo::meta::introspect::Introspect::<{}>::size()", item_type)]
    }
}

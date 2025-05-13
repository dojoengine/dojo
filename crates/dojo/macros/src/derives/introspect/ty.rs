use cairo_lang_syntax::node::ast::{Expr, TypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::TypedSyntaxNode;

use super::utils::{get_array_item_type, get_tuple_item_types, is_array, is_byte_array, is_tuple};

pub fn build_ty_from_type_clause(db: &dyn SyntaxGroup, type_clause: &TypeClause) -> String {
    match type_clause.ty(db) {
        Expr::Path(path) => {
            let path_type = path.as_syntax_node().get_text_without_trivia(db);
            build_item_ty_from_type(&path_type)
        }
        Expr::Tuple(expr) => {
            let tuple_type = expr.as_syntax_node().get_text_without_trivia(db);
            build_tuple_ty_from_type(&tuple_type)
        }
        _ => {
            // diagnostic message already handled in layout building
            "ERROR".to_string()
        }
    }
}

pub fn build_item_ty_from_type(item_type: &String) -> String {
    if is_array(item_type) {
        let array_item_type = get_array_item_type(item_type);
        format!(
            "dojo::meta::introspect::Ty::Array(
                array![
                {}
                ].span()
            )",
            build_item_ty_from_type(&array_item_type)
        )
    } else if is_byte_array(item_type) {
        "dojo::meta::introspect::Ty::ByteArray".to_string()
    } else if is_tuple(item_type) {
        build_tuple_ty_from_type(item_type)
    } else {
        format!("dojo::meta::introspect::Introspect::<{}>::ty()", item_type)
    }
}

pub fn build_tuple_ty_from_type(item_type: &str) -> String {
    let tuple_items = get_tuple_item_types(item_type)
        .iter()
        .map(build_item_ty_from_type)
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "dojo::meta::introspect::Ty::Tuple(
            array![
            {}
            ].span()
        )",
        tuple_items
    )
}

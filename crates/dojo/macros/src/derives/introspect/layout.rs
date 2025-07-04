use cairo_lang_macro::Diagnostic;
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::ast::{Expr, TypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::TypedSyntaxNode;

use super::utils::{is_array, is_byte_array, is_option, is_tuple};
use crate::derives::introspect::utils::{
    extract_fixed_array_type, get_tuple_item_types, is_fixed_size_array,
};
use crate::helpers::DiagnosticsExt;

/// Build a field layout describing the provided type clause.
pub(crate) fn get_layout_from_type_clause(
    db: &SimpleParserDatabase,
    diagnostics: &mut Vec<Diagnostic>,
    type_clause: &TypeClause,
) -> String {
    let type_str = match type_clause.ty(db) {
        Expr::Path(path) => path.as_syntax_node().get_text_without_trivia(db),
        Expr::Tuple(expr) => expr.as_syntax_node().get_text_without_trivia(db),
        Expr::FixedSizeArray(expr) => expr.as_syntax_node().get_text_without_trivia(db),
        _ => {
            diagnostics.push_error("Unexpected expression for variant data type.".to_string());
            return "".to_string();
        }
    };

    format!("dojo::meta::introspect::Introspect::<{}>::layout()", type_str)
}

pub fn is_custom_layout(layout: &str) -> bool {
    layout.starts_with("dojo::meta::introspect::Introspect::")
}

pub fn generate_cairo_code_for_fixed_layout_with_custom_types(layouts: &[String]) -> String {
    let layouts_repr = layouts
        .iter()
        .map(|l| {
            if is_custom_layout(l) {
                l.to_string()
            } else {
                format!("dojo::meta::Layout::Fixed(array![{l}].span())")
            }
        })
        .collect::<Vec<_>>()
        .join(",\n");

    format!(
        "let layouts = array![
            {layouts_repr}
        ];
        let mut merged_layout = ArrayTrait::<u8>::new();

        for layout in layouts {{
            match layout {{
                dojo::meta::Layout::Fixed(l) => merged_layout.append_span(l),
                _ => panic!(\"A packed model layout must contain Fixed layouts only.\"),
            }};
        }};

        dojo::meta::Layout::Fixed(merged_layout.span())
        ",
    )
}

//
pub fn get_packed_field_layout_from_type_clause(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<Diagnostic>,
    type_clause: &TypeClause,
) -> Vec<String> {
    match type_clause.ty(db) {
        Expr::Path(path) => {
            let path_type = path.as_syntax_node().get_text_without_trivia(db);
            get_packed_item_layout_from_type(diagnostics, path_type.trim())
        }
        Expr::Tuple(expr) => {
            let tuple_type = expr.as_syntax_node().get_text_without_trivia(db);
            get_packed_tuple_layout_from_type(diagnostics, &tuple_type)
        }
        Expr::FixedSizeArray(expr) => {
            let arr_type = expr.as_syntax_node().get_text_without_trivia(db);
            get_packed_item_layout_from_type(diagnostics, &arr_type)
        }
        _ => {
            diagnostics.push_error("Unexpected expression for variant data type.".to_string());
            vec![]
        }
    }
}

//
pub fn get_packed_item_layout_from_type(
    diagnostics: &mut Vec<Diagnostic>,
    item_type: &str,
) -> Vec<String> {
    if is_array(item_type) || is_byte_array(item_type) {
        diagnostics.push_error("Array field cannot be packed.".to_string());
        vec![]
    } else if is_tuple(item_type) {
        get_packed_tuple_layout_from_type(diagnostics, item_type)
    } else if is_fixed_size_array(item_type) {
        get_packed_fixed_array_layout_from_type(diagnostics, item_type)
    } else if is_option(item_type) {
        diagnostics.push_error(format!("{item_type} cannot be packed."));
        vec!["ERROR".to_string()]
    } else {
        // as we cannot verify that an enum/struct custom type is packable,
        // we suppose it is and let the user verify this.
        // If it's not the case, the Dojo model layout function will panic.
        vec![format!("dojo::meta::introspect::Introspect::<{}>::layout()", item_type)]
    }
}

//
pub fn get_packed_tuple_layout_from_type(
    diagnostics: &mut Vec<Diagnostic>,
    item_type: &str,
) -> Vec<String> {
    let mut layouts = vec![];

    for item in get_tuple_item_types(item_type).iter() {
        let layout = get_packed_item_layout_from_type(diagnostics, item);
        layouts.push(layout);
    }

    layouts.into_iter().flatten().collect::<Vec<_>>()
}

pub fn get_packed_fixed_array_layout_from_type(
    diagnostics: &mut Vec<Diagnostic>,
    item_type: &str,
) -> Vec<String> {
    let (item_type, size) = extract_fixed_array_type(item_type);
    let layout = get_packed_item_layout_from_type(diagnostics, &item_type);

    let mut layouts = vec![];

    for _ in 0..size {
        layouts.push(layout.clone());
    }

    layouts.into_iter().flatten().collect::<Vec<_>>()
}

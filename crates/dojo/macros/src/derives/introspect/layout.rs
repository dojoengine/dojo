use cairo_lang_macro::Diagnostic;
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::ast::{Expr, TypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::TypedSyntaxNode;

use super::utils::{
    get_array_item_type, get_tuple_item_types, is_array, is_byte_array, is_option, is_tuple,
    is_unsupported_option_type,
};
use crate::helpers::DiagnosticsExt;

/// Build a field layout describing the provided type clause.
pub(crate) fn get_layout_from_type_clause(
    db: &SimpleParserDatabase,
    diagnostics: &mut Vec<Diagnostic>,
    type_clause: &TypeClause,
) -> String {
    match type_clause.ty(db) {
        Expr::Path(path) => {
            let path_type = path.as_syntax_node().get_text_without_trivia(db);
            build_item_layout_from_type(diagnostics, &path_type)
        }
        Expr::Tuple(expr) => {
            let tuple_type = expr.as_syntax_node().get_text_without_trivia(db);
            build_tuple_layout_from_type(diagnostics, &tuple_type)
        }
        _ => {
            diagnostics.push_error("Unexpected expression for variant data type.".to_string());
            "".to_string()
        }
    }
}

/// Build the array layout describing the provided array type.
/// item_type could be something like `Array<u128>` for example.
pub fn build_array_layout_from_type(diagnostics: &mut Vec<Diagnostic>, item_type: &str) -> String {
    let array_item_type = get_array_item_type(item_type);

    if is_tuple(&array_item_type) {
        let layout = build_item_layout_from_type(diagnostics, &array_item_type);
        format!(
            "dojo::meta::Layout::Array(
                array![
                    {layout}
                ].span()
            )"
        )
    } else if is_array(&array_item_type) {
        let layout = build_array_layout_from_type(diagnostics, &array_item_type);
        format!(
            "dojo::meta::Layout::Array(
                array![
                    {layout}
                ].span()
            )"
        )
    } else {
        format!("dojo::meta::introspect::Introspect::<{}>::layout()", item_type)
    }
}

/// Build the tuple layout describing the provided tuple type.
/// item_type could be something like (u8, u32, u128) for example.
pub fn build_tuple_layout_from_type(diagnostics: &mut Vec<Diagnostic>, item_type: &str) -> String {
    let mut tuple_items = vec![];

    for item in get_tuple_item_types(item_type).iter() {
        let layout = build_item_layout_from_type(diagnostics, item);
        tuple_items.push(layout);
    }

    format!(
        "dojo::meta::Layout::Tuple(
            array![
            {}
            ].span()
        )",
        tuple_items.join(",\n")
    )
}

/// Build the layout describing the provided type.
/// item_type could be any type (array, tuple, struct, ...)
pub fn build_item_layout_from_type(diagnostics: &mut Vec<Diagnostic>, item_type: &str) -> String {
    if is_array(item_type) {
        build_array_layout_from_type(diagnostics, item_type)
    } else if is_tuple(item_type) {
        build_tuple_layout_from_type(diagnostics, item_type)
    } else {
        // For Option<T>, T cannot be a tuple
        if is_unsupported_option_type(item_type) {
            diagnostics.push_error(
                "Option<T> cannot be used with tuples. Prefer using a struct.".to_string(),
            );
        }

        // `usize` is forbidden because its size is architecture-dependent
        if item_type == "usize" {
            diagnostics.push_error(
                "Use u32 rather than usize as usize size is architecture dependent.".to_string(),
            );
        }

        format!("dojo::meta::introspect::Introspect::<{}>::layout()", item_type)
    }
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

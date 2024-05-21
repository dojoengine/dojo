use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::{Expr, ItemEnum, ItemStruct, OptionTypeClause, TypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ids, Terminal, TypedSyntaxNode};
use starknet::core::utils::get_selector_from_name;

use super::utils::{
    get_array_item_type, get_tuple_item_types, is_array, is_byte_array, is_tuple,
    is_unsupported_option_type, primitive_type_introspection,
};

/// build the full layout for every field in the Struct.
pub fn build_field_layouts(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    struct_ast: &ItemStruct,
) -> String {
    struct_ast
        .members(db)
        .elements(db)
        .iter()
        .filter_map(|m| {
            if m.has_attr(db, "key") {
                return None;
            }

            let field_name = m.name(db).text(db);
            let field_selector = get_selector_from_name(field_name.as_str()).unwrap().to_string();
            let field_layout = get_layout_from_type_clause(db, diagnostics, &m.type_clause(db));
            Some(format!(
                "dojo::database::introspect::FieldLayout {{
                    selector: {field_selector},
                    layout: {field_layout}
                }}"
            ))
        })
        .collect::<Vec<_>>()
        .join(",\n")
}

/// build the full layout for every variant in the Enum.
/// Note that every variant may have a different associated data type.
pub fn build_variant_layouts(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    enum_ast: &ItemEnum,
) -> String {
    enum_ast
        .variants(db)
        .elements(db)
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let selector = format!("{i}");

            let variant_layout = match v.type_clause(db) {
                OptionTypeClause::Empty(_) => "".to_string(),
                OptionTypeClause::TypeClause(type_clause) => {
                    get_layout_from_type_clause(db, diagnostics, &type_clause)
                }
            };

            format!(
                "dojo::database::introspect::FieldLayout {{
                    selector: {selector},
                    layout: dojo::database::introspect::Layout::Tuple(
                        array![
                            dojo::database::introspect::Layout::Fixed(array![8].span()),
                            {variant_layout}
                        ].span()
                    )
                }}"
            )
        })
        .collect::<Vec<_>>()
        .join(",\n")
}

/// Build a field layout describing the provided type clause.
pub fn get_layout_from_type_clause(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    type_clause: &TypeClause,
) -> String {
    match type_clause.ty(db) {
        Expr::Path(path) => {
            let path_type = path.as_syntax_node().get_text(db);
            build_item_layout_from_type(diagnostics, type_clause.stable_ptr().0, &path_type)
        }
        Expr::Tuple(expr) => {
            let tuple_type = expr.as_syntax_node().get_text(db);
            build_tuple_layout_from_type(diagnostics, type_clause.stable_ptr().0, &tuple_type)
        }
        _ => {
            diagnostics.push(PluginDiagnostic {
                stable_ptr: type_clause.stable_ptr().0,
                message: "Unexpected expression for variant data type.".to_string(),
                severity: Severity::Error,
            });
            "ERROR".to_string()
        }
    }
}

/// Build the array layout describing the provided array type.
/// item_type could be something like Array<u128> for example.
pub fn build_array_layout_from_type(
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: ids::SyntaxStablePtrId,
    item_type: &str,
) -> String {
    let array_item_type = get_array_item_type(item_type);

    if is_tuple(&array_item_type) {
        format!(
            "dojo::database::introspect::Layout::Array(
                array![
                    {}
                ].span()
            )",
            build_item_layout_from_type(diagnostics, diagnostic_item, &array_item_type)
        )
    } else if is_array(&array_item_type) {
        format!(
            "dojo::database::introspect::Layout::Array(
                array![
                    {}
                ].span()
            )",
            build_array_layout_from_type(diagnostics, diagnostic_item, &array_item_type)
        )
    } else {
        format!("dojo::database::introspect::Introspect::<{}>::layout()", item_type)
    }
}

/// Build the tuple layout describing the provided tuple type.
/// item_type could be something like (u8, u32, u128) for example.
pub fn build_tuple_layout_from_type(
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: ids::SyntaxStablePtrId,
    item_type: &str,
) -> String {
    let tuple_items = get_tuple_item_types(item_type)
        .iter()
        .map(|x| build_item_layout_from_type(diagnostics, diagnostic_item, x))
        .collect::<Vec<_>>()
        .join(",\n");
    format!(
        "dojo::database::introspect::Layout::Tuple(
            array![
            {}
            ].span()
        )",
        tuple_items
    )
}

/// Build the layout describing the provided type.
/// item_type could be any type (array, tuple, struct, ...)
pub fn build_item_layout_from_type(
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: ids::SyntaxStablePtrId,
    item_type: &str,
) -> String {
    if is_array(item_type) {
        build_array_layout_from_type(diagnostics, diagnostic_item, item_type)
    } else if is_tuple(item_type) {
        build_tuple_layout_from_type(diagnostics, diagnostic_item, item_type)
    } else {
        // For Option<T>, T cannot be a tuple
        if is_unsupported_option_type(item_type) {
            diagnostics.push(PluginDiagnostic {
                stable_ptr: diagnostic_item,
                message: "Option<T> cannot be used with tuples. Prefer using a struct.".into(),
                severity: Severity::Error,
            });
        }

        format!("dojo::database::introspect::Introspect::<{}>::layout()", item_type)
    }
}

pub fn build_packed_struct_layout(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    struct_ast: &ItemStruct,
) -> String {
    struct_ast
        .members(db)
        .elements(db)
        .iter()
        .filter_map(|m| {
            if m.has_attr(db, "key") {
                return None;
            }

            Some(get_packed_field_layout_from_type_clause(db, diagnostics, &m.type_clause(db)))
        })
        .collect::<Vec<_>>()
        .join(",")
}

//
pub fn build_packed_enum_layout(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    enum_ast: &ItemEnum,
) -> String {
    let variant_layouts = enum_ast
        .variants(db)
        .elements(db)
        .iter()
        .map(|v| match v.type_clause(db) {
            OptionTypeClause::Empty(_) => "".to_string(),
            OptionTypeClause::TypeClause(type_clause) => {
                get_packed_field_layout_from_type_clause(db, diagnostics, &type_clause)
            }
        })
        .collect::<Vec<_>>();

    if variant_layouts.is_empty() {
        return "8".to_string();
    }

    format!("8,{}", variant_layouts[0])
}

//
pub fn get_packed_field_layout_from_type_clause(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    type_clause: &TypeClause,
) -> String {
    match type_clause.ty(db) {
        Expr::Path(path) => {
            let path_type = path.as_syntax_node().get_text(db);
            get_packed_item_layout_from_type(
                diagnostics,
                type_clause.stable_ptr().0,
                path_type.trim(),
            )
        }
        Expr::Tuple(expr) => {
            let tuple_type = expr.as_syntax_node().get_text(db);
            get_packed_tuple_layout_from_type(diagnostics, type_clause.stable_ptr().0, &tuple_type)
        }
        _ => {
            diagnostics.push(PluginDiagnostic {
                stable_ptr: type_clause.stable_ptr().0,
                message: "Unexpected expression for variant data type.".to_string(),
                severity: Severity::Error,
            });
            "ERROR".to_string()
        }
    }
}

//
pub fn get_packed_item_layout_from_type(
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: ids::SyntaxStablePtrId,
    item_type: &str,
) -> String {
    if is_array(item_type) || is_byte_array(item_type) {
        diagnostics.push(PluginDiagnostic {
            stable_ptr: diagnostic_item,
            message: "Array field cannot be packed.".into(),
            severity: Severity::Error,
        });
        "ERROR".to_string()
    } else if is_tuple(item_type) {
        get_packed_tuple_layout_from_type(diagnostics, diagnostic_item, item_type)
    } else {
        let primitives = primitive_type_introspection();

        if let Some(p) = primitives.get(item_type) {
            p.1.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
        } else {
            diagnostics.push(PluginDiagnostic {
                stable_ptr: diagnostic_item,
                message: "For now, field with custom type cannot be packed".into(),
                severity: Severity::Error,
            });
            "ERROR".to_string()
        }
    }
}

//
pub fn get_packed_tuple_layout_from_type(
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: ids::SyntaxStablePtrId,
    item_type: &str,
) -> String {
    get_tuple_item_types(item_type)
        .iter()
        .map(|x| get_packed_item_layout_from_type(diagnostics, diagnostic_item, x))
        .collect::<Vec<_>>()
        .join(",")
}

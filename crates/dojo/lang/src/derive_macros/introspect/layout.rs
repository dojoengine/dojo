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
            let field_selector = get_selector_from_name(&field_name.to_string()).unwrap();
            let field_layout = get_layout_from_type_clause(db, diagnostics, &m.type_clause(db));
            Some(format!(
                "dojo::meta::FieldLayout {{
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
                OptionTypeClause::Empty(_) => {
                    "dojo::meta::Layout::Fixed(array![].span())".to_string()
                }
                OptionTypeClause::TypeClause(type_clause) => {
                    get_layout_from_type_clause(db, diagnostics, &type_clause)
                }
            };

            format!(
                "dojo::meta::FieldLayout {{
                    selector: {selector},
                    layout: {variant_layout}
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
/// item_type could be something like `Array<u128>` for example.
pub fn build_array_layout_from_type(
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: ids::SyntaxStablePtrId,
    item_type: &str,
) -> String {
    let array_item_type = get_array_item_type(item_type);

    if is_tuple(&array_item_type) {
        format!(
            "dojo::meta::Layout::Array(
                array![
                    {}
                ].span()
            )",
            build_item_layout_from_type(diagnostics, diagnostic_item, &array_item_type)
        )
    } else if is_array(&array_item_type) {
        format!(
            "dojo::meta::Layout::Array(
                array![
                    {}
                ].span()
            )",
            build_array_layout_from_type(diagnostics, diagnostic_item, &array_item_type)
        )
    } else {
        format!(
            "dojo::meta::introspect::Introspect::<{}>::layout()",
            item_type
        )
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
        "dojo::meta::Layout::Tuple(
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

        format!(
            "dojo::meta::introspect::Introspect::<{}>::layout()",
            item_type
        )
    }
}

pub fn is_custom_layout(layout: &str) -> bool {
    layout.starts_with("dojo::meta::introspect::Introspect::")
}

pub fn build_packed_struct_layout(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    struct_ast: &ItemStruct,
) -> String {
    let layouts = struct_ast
        .members(db)
        .elements(db)
        .iter()
        .filter_map(|m| {
            if m.has_attr(db, "key") {
                return None;
            }

            Some(get_packed_field_layout_from_type_clause(
                db,
                diagnostics,
                &m.type_clause(db),
            ))
        })
        .flatten()
        .collect::<Vec<_>>();

    if layouts.iter().any(|v| is_custom_layout(v.as_str())) {
        generate_cairo_code_for_fixed_layout_with_custom_types(&layouts)
    } else {
        format!(
            "dojo::meta::Layout::Fixed(
            array![
            {}
            ].span()
        )",
            layouts.join(",")
        )
    }
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
        "let mut layouts = array![
            {layouts_repr}
        ];
        let mut merged_layout = ArrayTrait::<u8>::new();

        loop {{
            match ArrayTrait::pop_front(ref layouts) {{
                Option::Some(mut layout) => {{
                    match layout {{
                        dojo::meta::Layout::Fixed(mut l) => {{
                            loop {{
                                match SpanTrait::pop_front(ref l) {{
                                    Option::Some(x) => merged_layout.append(*x),
                                    Option::None(_) => {{ break; }}
                                }};
                            }};
                        }},
                        _ => panic!(\"A packed model layout must contain Fixed layouts only.\"),
                    }};
                }},
                Option::None(_) => {{ break; }}
            }};
        }};

        dojo::meta::Layout::Fixed(merged_layout.span())
        ",
    )
}

//
pub fn build_packed_enum_layout(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    enum_ast: &ItemEnum,
) -> String {
    // to be packable, all variants data must have the same size.
    // as this point has already been checked before calling `build_packed_enum_layout`,
    // just use the first variant to generate the fixed layout.
    let elements = enum_ast.variants(db).elements(db);
    let mut variant_layout = if elements.is_empty() {
        vec![]
    } else {
        match elements.first().unwrap().type_clause(db) {
            OptionTypeClause::Empty(_) => vec![],
            OptionTypeClause::TypeClause(type_clause) => {
                get_packed_field_layout_from_type_clause(db, diagnostics, &type_clause)
            }
        }
    };

    // don't forget the store the variant value
    variant_layout.insert(0, "8".to_string());

    if variant_layout.iter().any(|v| is_custom_layout(v.as_str())) {
        generate_cairo_code_for_fixed_layout_with_custom_types(&variant_layout)
    } else {
        format!(
            "dojo::meta::Layout::Fixed(
                array![
                {}
                ].span()
            )",
            variant_layout.join(",")
        )
    }
}

//
pub fn get_packed_field_layout_from_type_clause(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    type_clause: &TypeClause,
) -> Vec<String> {
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
            vec!["ERROR".to_string()]
        }
    }
}

//
pub fn get_packed_item_layout_from_type(
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: ids::SyntaxStablePtrId,
    item_type: &str,
) -> Vec<String> {
    if is_array(item_type) || is_byte_array(item_type) {
        diagnostics.push(PluginDiagnostic {
            stable_ptr: diagnostic_item,
            message: "Array field cannot be packed.".into(),
            severity: Severity::Error,
        });
        vec!["ERROR".to_string()]
    } else if is_tuple(item_type) {
        get_packed_tuple_layout_from_type(diagnostics, diagnostic_item, item_type)
    } else {
        let primitives = primitive_type_introspection();

        if let Some(p) = primitives.get(item_type) {
            vec![p
                .1
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(",")]
        } else {
            // as we cannot verify that an enum/struct custom type is packable,
            // we suppose it is and let the user verify this.
            // If it's not the case, the Dojo model layout function will panic.
            vec![format!(
                "dojo::meta::introspect::Introspect::<{}>::layout()",
                item_type
            )]
        }
    }
}

//
pub fn get_packed_tuple_layout_from_type(
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: ids::SyntaxStablePtrId,
    item_type: &str,
) -> Vec<String> {
    get_tuple_item_types(item_type)
        .iter()
        .flat_map(|x| get_packed_item_layout_from_type(diagnostics, diagnostic_item, x))
        .collect::<Vec<_>>()
}

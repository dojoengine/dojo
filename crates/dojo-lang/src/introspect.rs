use std::collections::HashMap;

use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::{
    Expr, GenericParam, ItemEnum, ItemStruct, OptionTypeClause, OptionWrappedGenericParamList,
    TypeClause,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::ids;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_world::manifest::Member;
use itertools::Itertools;
use starknet::core::utils::get_selector_from_name;

#[derive(PartialEq)]
enum CompositeType {
    Enum,
    Struct,
}

#[derive(Clone, Default)]
struct TypeIntrospection(usize, Vec<usize>);

// Provides type introspection information for primitive types
fn primitive_type_introspection() -> HashMap<String, TypeIntrospection> {
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

/// Check if the provided type is an unsupported Option<T>,
/// because tuples are not supported with Option.
fn is_unsupported_option_type(ty: &String) -> bool {
    ty.starts_with("Option<(")
}

fn is_byte_array(ty: &String) -> bool {
    ty.eq("ByteArray")
}

fn is_array(ty: &String) -> bool {
    ty.starts_with("Array<") || ty.starts_with("Span<")
}

fn is_tuple(ty: &String) -> bool {
    ty.starts_with("(")
}

fn get_array_item_type(ty: &String) -> String {
    if ty.starts_with("Array<") {
        ty.trim().strip_prefix("Array<").unwrap().strip_suffix('>').unwrap().to_string()
    } else {
        ty.trim().strip_prefix("Span<").unwrap().strip_suffix('>').unwrap().to_string()
    }
}

/// split a tuple in array of items (nested tuples are not splitted).
/// example (u8, (u16, u32), u128) -> ["u8", "(u16, u32)", "u128"]
fn get_tuple_item_types(ty: &String) -> Vec<String> {
    let tuple_str = ty
        .trim()
        .strip_prefix("(")
        .unwrap()
        .strip_suffix(")")
        .unwrap()
        .to_string()
        .replace(" ", "");
    let mut items = vec![];
    let mut current_item = "".to_string();
    let mut level = 0;

    for c in tuple_str.chars() {
        if c == ',' {
            if level > 0 {
                current_item.push(c);
            }

            if level == 0 && current_item.len() > 0 {
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

    if current_item.len() > 0 {
        items.push(current_item);
    }

    items
}

/// Build the array layout describing the provided array type.
/// item_type could be something like Array<u128> for example.
fn build_array_layout_from_type(
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: ids::SyntaxStablePtrId,
    item_type: &String,
) -> String {
    let array_item_type = get_array_item_type(item_type);

    if is_tuple(&array_item_type) {
        format!(
            "dojo::database::introspect::Layout::Array(
                array![
                    dojo::database::introspect::FieldLayout {{
                        selector: '',
                        layout: {}
                    }}
                ].span()
            )",
            build_item_layout_from_type(diagnostics, diagnostic_item, &array_item_type)
        )
    } else if is_array(&array_item_type) {
        format!(
            "dojo::database::introspect::Layout::Array(
                array![
                    dojo::database::introspect::FieldLayout {{
                        selector: '',
                        layout: {}
                    }}
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
fn build_tuple_layout_from_type(
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: ids::SyntaxStablePtrId,
    item_type: &String,
) -> String {
    let tuple_items = get_tuple_item_types(item_type)
        .iter()
        .map(|x| {
            format!(
                "dojo::database::introspect::FieldLayout {{
                        selector: '',
                        layout: {}
                    }}",
                build_item_layout_from_type(diagnostics, diagnostic_item, &x)
            )
        })
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
fn build_item_layout_from_type(
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: ids::SyntaxStablePtrId,
    item_type: &String,
) -> String {
    if is_array(item_type) {
        build_array_layout_from_type(diagnostics, diagnostic_item, item_type)
    } else if is_tuple(&item_type) {
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

/// Build a field layout describing the provided type clause.
fn get_field_layout_from_type_clause(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    selector: String,
    type_clause: &TypeClause,
) -> String {
    let field_layout = match type_clause.ty(db) {
        Expr::Path(path) => {
            let path_type = path.as_syntax_node().get_text(db);
            build_item_layout_from_type(diagnostics, type_clause.stable_ptr().untyped(), &path_type)
        }
        Expr::Tuple(expr) => {
            let tuple_type = expr.as_syntax_node().get_text(db);
            build_tuple_layout_from_type(
                diagnostics,
                type_clause.stable_ptr().untyped(),
                &tuple_type,
            )
        }
        _ => {
            diagnostics.push(PluginDiagnostic {
                stable_ptr: type_clause.stable_ptr().untyped(),
                message: "Unexpected expression for variant data type.".to_string(),
                severity: Severity::Error,
            });
            "ERROR".to_string()
        }
    };

    format!(
        "dojo::database::introspect::FieldLayout {{
            selector: {selector},
            layout: {field_layout}
        }}"
    )
}

/// build the full layout for every variant in the Enum.
/// Note that every variant may have a different associated data type.
fn build_variant_layouts(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    enum_ast: ItemEnum,
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
                OptionTypeClause::TypeClause(type_clause) => get_field_layout_from_type_clause(
                    db,
                    diagnostics,
                    "''".to_string(),
                    &type_clause,
                ),
            };

            format!(
                "dojo::database::introspect::FieldLayout {{
                    selector: {selector},
                    layout: dojo::database::introspect::Layout::Tuple(
                        array![
                            // variant value
                            dojo::database::introspect::FieldLayout {{
                                selector: '',
                                layout: dojo::database::introspect::Layout::Fixed(array![8].span())
                            }},
                            {variant_layout}
                        ].span()
                    )
                }}"
            )
        })
        .collect::<Vec<_>>()
        .join(",\n")
}

/// build the full layout for every field in the Struct.
fn build_field_layouts(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    struct_ast: ItemStruct,
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

            Some(get_field_layout_from_type_clause(
                db,
                diagnostics,
                field_selector,
                &m.type_clause(db),
            ))
        })
        .collect::<Vec<_>>()
        .join(",\n")
}

// Extract generic type information and build the
// type and impl information to add to the generated introspect
fn build_generic_types_and_impls(
    db: &dyn SyntaxGroup,
    generic_params: OptionWrappedGenericParamList,
) -> (Vec<String>, String) {
    let generic_types =
        if let OptionWrappedGenericParamList::WrappedGenericParamList(params) = generic_params {
            params
                .generic_params(db)
                .elements(db)
                .iter()
                .filter_map(|el| {
                    if let GenericParam::Type(typ) = el {
                        Some(typ.name(db).text(db).to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

    let generic_impls = generic_types
        .iter()
        .map(|g| format!("{g}, impl {g}Introspect: dojo::database::introspect::Introspect<{g}>"))
        .collect::<Vec<_>>()
        .join(", ");

    (generic_types, generic_impls)
}

/// Handle the introspection of a Struct
pub fn handle_introspect_struct(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    struct_ast: ItemStruct,
) -> RewriteNode {
    let struct_name = struct_ast.name(db).text(db).into();

    // TODO: compute size (is it really interesting ?)
    let struct_size = "Option::None".to_string();

    let (generic_types, generic_impls) =
        build_generic_types_and_impls(db, struct_ast.generic_params(db));

    let field_layouts = build_field_layouts(db, diagnostics, struct_ast);

    generate_introspect(
        &struct_name,
        &struct_size,
        &generic_types,
        generic_impls,
        &"Struct".to_string(),
        &field_layouts,
    )
}

/// Handle the introspection of a Enum
pub fn handle_introspect_enum(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    enum_ast: ItemEnum,
) -> RewriteNode {
    let enum_name = enum_ast.name(db).text(db).into();

    // TODO: compute size (is it really interesting ?)
    let enum_size = "Option::None".to_string();

    let (generic_types, generic_impls) =
        build_generic_types_and_impls(db, enum_ast.generic_params(db));

    let variant_layouts = build_variant_layouts(db, diagnostics, enum_ast);

    generate_introspect(
        &enum_name,
        &enum_size,
        &generic_types,
        generic_impls,
        &"Enum".to_string(),
        &variant_layouts,
    )
}

/// generate the type introspection
fn generate_introspect(
    name: &String,
    size: &String,
    generic_types: &Vec<String>,
    generic_impls: String,
    layout_type: &String,
    layouts: &String,
) -> RewriteNode {
    RewriteNode::interpolate_patched(
        "
impl $name$Introspect<$generics$> of dojo::database::introspect::Introspect<$name$<$generics_types$>> {
    #[inline(always)]
    fn size() -> Option<usize> {
        $size$
    }

    #[inline(always)]
    fn layout() -> dojo::database::introspect::Layout {
        dojo::database::introspect::Layout::$layout_type$(
            array![
            $layouts$
            ].span()
        )
    }

    #[inline(always)]
    fn ty() -> dojo::database::introspect::Ty {
        // TODO: Not used anymore => to remove
        dojo::database::introspect::Ty::Primitive('u8')
    }
}
        ",
        &UnorderedHashMap::from([
            ("name".to_string(), RewriteNode::Text(name.to_string())),
            ("generics".to_string(), RewriteNode::Text(generic_impls)),
            ("generics_types".to_string(), RewriteNode::Text(generic_types.join(", "))),
            ("size".to_string(), RewriteNode::Text(size.to_string())),
            ("layout_type".to_string(), RewriteNode::Text(layout_type.to_string())),
            ("layouts".to_string(), RewriteNode::Text(layouts.to_string())),
        ]),
    )
}

/// =============================================================
///
/// TODO: to remove if we do not use Ty and size anymore
///
///
/// =============================================================

/// A handler for Dojo code derives Introspect for a struct
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the struct.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_introspect_struct_old(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    struct_ast: ItemStruct,
) -> RewriteNode {
    let name = struct_ast.name(db).text(db).into();

    let mut member_types: Vec<String> = vec![];
    let primitive_sizes = primitive_type_introspection();

    let members: Vec<_> = struct_ast
        .members(db)
        .elements(db)
        .iter()
        .map(|member| {
            let is_key = member.has_attr(db, "key");

            let mut ty =
                member.type_clause(db).ty(db).as_syntax_node().get_text(db).trim().to_string();
            let member_name = member.name(db).text(db).to_string();
            let attrs = if is_key { vec!["'key'"] } else { vec![] };

            if primitive_sizes.get(&ty).is_some() {
                // It's a primitive type
                member_types.push(format!(
                    "dojo::database::introspect::serialize_member(
                     @dojo::database::introspect::Member {{
                name: '{member_name}',
                ty: dojo::database::introspect::Ty::Primitive('{ty}'),
                attrs: array![{}].span()
            }})",
                    attrs.join(","),
                ));
            } else if is_byte_array(&ty) {
                // TODO for Ty
            } else if is_array(&ty) {
                let item_type = get_array_item_type(&ty);

                member_types.push(format!(
                    "dojo::database::introspect::serialize_member(
                        @dojo::database::introspect::Member {{
                        name: '{member_name}',
                        ty: dojo::database::introspect::Ty::DynamicSizeArray,
                        attrs: array![{}].span()
                    }})",
                    attrs.join(","),
                ));

                ty = format!("dyn_array__{}", item_type);
            } else if let Expr::Tuple(tuple) = member.type_clause(db).ty(db) {
                let tuple_items = (*tuple.expressions(db))
                    .elements(db)
                    .iter()
                    .filter_map(|e| {
                        let e_ty = e.as_syntax_node().get_text(db).trim().to_string();
                        if primitive_sizes.get(&e_ty).is_some() {
                            Some(format!(
                                "dojo::database::introspect::serialize_member_type(
                                    @dojo::database::introspect::Ty::Primitive('{e_ty}')
                                )"
                            ))
                        } else {
                            // No need to handle other types as the Ty will be removed soon
                            None
                        }
                    })
                    .collect::<Vec<_>>();

                // Tuple
                member_types.push(format!(
                    "dojo::database::introspect::serialize_member(
                        @dojo::database::introspect::Member {{
                        name: '{member_name}',
                        ty: dojo::database::introspect::Ty::Tuple(
                            array![
                                {}
                            ].span()
                        ),
                        attrs: array![{}].span()
                    }})",
                    tuple_items.join(",\n"),
                    attrs.join(","),
                ));
            } else {
                // It's a custom struct/enum
                member_types.push(format!(
                    "dojo::database::introspect::serialize_member(
                        @dojo::database::introspect::Member {{
                        name: '{member_name}',
                        ty: dojo::database::introspect::Introspect::<{ty}>::ty(),
                        attrs: array![{}].span()
                    }})",
                    attrs.join(","),
                ));
            }

            Member { name: member_name, ty, key: is_key }
        })
        .collect::<_>();

    let type_ty = format!(
        "dojo::database::introspect::Ty::Struct(dojo::database::introspect::Struct {{
            name: '{name}',
            attrs: array![].span(),
            children: array![
                {}\n
            ].span()
        }})",
        member_types.join(",\n")
    );

    handle_introspect_internal(
        db,
        name,
        struct_ast.generic_params(db),
        CompositeType::Struct,
        0,
        type_ty,
        members,
        diagnostics,
        struct_ast.name(db).stable_ptr().untyped(),
    )
}

/// A handler for Dojo code derives Introspect for an enum
/// Parameters:
/// * db: The semantic database.
/// * enum_ast: The AST of the enum.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_introspect_enum_old(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    enum_ast: ItemEnum,
) -> RewriteNode {
    let primitive_sizes = primitive_type_introspection();

    let name = enum_ast.name(db).text(db).into();

    let variant_type = enum_ast.variants(db).elements(db).first().unwrap().type_clause(db);
    let variant_type_text = variant_type.as_syntax_node().get_text(db);
    let variant_type_text = variant_type_text.trim();
    let mut variant_type_arr = vec![];

    if let OptionTypeClause::TypeClause(types_tuple) = variant_type {
        if let Expr::Tuple(paren_list) = types_tuple.ty(db) {
            let args = (*paren_list.expressions(db)).elements(db);
            args.iter().for_each(|arg| {
                let ty_name = arg.as_syntax_node().get_text(db);
                let is_primitive = primitive_sizes.get(&ty_name).is_some();

                variant_type_arr.push(handle_enum_arm_type(&ty_name, is_primitive));
            });
        } else if let Expr::Path(type_path) = types_tuple.ty(db) {
            let ty_name = type_path.as_syntax_node().get_text(db);
            let is_primitive = primitive_sizes.get(&ty_name).is_some();

            if is_array(&ty_name) {
                diagnostics.push(PluginDiagnostic {
                    stable_ptr: types_tuple.stable_ptr().0,
                    message: "Dynamic arrays are not supported.".to_string(),
                    severity: Severity::Error,
                });
            }

            variant_type_arr.push(handle_enum_arm_type(&ty_name, is_primitive));
        } else {
            diagnostics.push(PluginDiagnostic {
                stable_ptr: types_tuple.stable_ptr().0,
                message: "Only tuple and type paths are supported.".to_string(),
                severity: Severity::Error,
            });
        }
    }

    let members: Vec<_> = variant_type_arr
        .iter()
        .map(|(_, ty)| Member { name: ty.into(), ty: ty.into(), key: false })
        .collect_vec();

    let mut arms_ty: Vec<String> = vec![];

    // Add diagnostics for different Typeclauses.
    enum_ast.variants(db).elements(db).iter().for_each(|member| {
        let member_name = member.name(db).text(db);
        let member_type = member.type_clause(db).as_syntax_node();
        let member_type_text = member_type.get_text(db);
        if member_type_text.trim() != variant_type_text.trim() {
            diagnostics.push(PluginDiagnostic {
                stable_ptr: member_type.stable_ptr(),
                message: format!("Enum arms need to have same type - {}.", variant_type_text),
                severity: Severity::Error,
            });
        }

        // @TODO: Prepare type struct
        arms_ty.push(format!(
            "(
                    '{member_name}',
                    dojo::database::introspect::serialize_member_type(
                    @dojo::database::introspect::Ty::Tuple(array![{}].span()))
                )",
            if !variant_type_arr.is_empty() {
                let ty_cairo: Vec<_> =
                    variant_type_arr.iter().map(|(ty_cairo, _)| ty_cairo.to_string()).collect();
                ty_cairo.join(",\n")
            } else {
                "".to_string()
            }
        ));
    });

    let type_ty = format!(
        "dojo::database::introspect::Ty::Enum(
            dojo::database::introspect::Enum {{
                name: '{name}',
                attrs: array![].span(),
                children: array![
                {}\n
                ].span()
            }}
        )",
        arms_ty.join(",\n")
    );

    // Enums have 1 size and 8 bit layout by default
    let size_precompute = 1;
    handle_introspect_internal(
        db,
        name,
        enum_ast.generic_params(db),
        CompositeType::Enum,
        size_precompute,
        type_ty,
        members,
        diagnostics,
        enum_ast.stable_ptr().untyped(),
    )
}

fn handle_introspect_internal(
    db: &dyn SyntaxGroup,
    name: String,
    generics: OptionWrappedGenericParamList,
    composite_type: CompositeType,
    mut size_precompute: usize,
    type_ty: String,
    members: Vec<Member>,
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: ids::SyntaxStablePtrId,
) -> RewriteNode {
    let mut size = vec![];
    let mut dynamic_size = false;
    let primitive_sizes = primitive_type_introspection();
    let mut layout = match composite_type {
        CompositeType::Enum => {
            vec!["dojo::database::introspect::FieldLayout {
                    selector: '',
                    layout: dojo::database::introspect::Introspect::<u8>::layout()
                }"
            .to_string()]
        }
        CompositeType::Struct => vec![],
    };

    let generics = if let OptionWrappedGenericParamList::WrappedGenericParamList(params) = generics
    {
        params
            .generic_params(db)
            .elements(db)
            .iter()
            .filter_map(|el| {
                if let GenericParam::Type(typ) = el {
                    Some(typ.name(db).text(db).to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    let generic_impls = generics
        .iter()
        .map(|g| format!("{g}, impl {g}Introspect: dojo::database::introspect::Introspect<{g}>"))
        .collect::<Vec<_>>()
        .join(", ");

    members.iter().for_each(|m| {
        let primitive_intro = primitive_sizes.get(&m.ty);
        let mut attrs = vec![];

        if let Some(p_ty) = primitive_intro {
            // It's a primitive type
            if m.key {
                attrs.push("'key'");
            } else {
                size_precompute += p_ty.0;

                match composite_type {
                    CompositeType::Enum => layout.push(format!(
                        "dojo::database::introspect::FieldLayout {{
                            selector: '',
                            layout: dojo::database::introspect::Introspect::<{}>::layout()
                        }}",
                        m.ty
                    )),
                    CompositeType::Struct => {
                        let values =
                            p_ty.1.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(", ");

                        layout.push(format!(
                            "dojo::database::introspect::FieldLayout {{
                                selector: {},
                                layout: \
                             dojo::database::introspect::Layout::Fixed(array![{}].span())
                            }}",
                            get_selector_from_name(m.name.as_str()).unwrap(),
                            values
                        ))
                    }
                };
            }
        } else if m.ty.starts_with("ByteArray") {
            dynamic_size = true;

            match composite_type {
                CompositeType::Enum => {
                    layout.push(
                        "dojo::database::introspect::FieldLayout {
                            selector: '',
                            layout: dojo::database::introspect::Layout::ByteArray
                        }"
                        .to_string(),
                    );
                }
                CompositeType::Struct => {
                    layout.push(format!(
                        "dojo::database::introspect::FieldLayout {{
                            selector: {},
                            layout: dojo::database::introspect::Layout::ByteArray
                        }}",
                        get_selector_from_name(m.name.as_str()).unwrap()
                    ));
                }
            }
        } else if m.ty.starts_with("dyn_array__") {
            let item_type = m.ty.strip_prefix("dyn_array__").unwrap();
            dynamic_size = true;

            let array_layout = format!(
                "dojo::database::introspect::Layout::Array(
                array![
                    dojo::database::introspect::FieldLayout {{
                        selector: '',
                        layout: {}
                    }}
                ].span()
            )",
                build_item_layout_from_type(diagnostics, diagnostic_item, &item_type.to_string())
            );

            match composite_type {
                CompositeType::Enum => {
                    layout.push(array_layout);
                }
                CompositeType::Struct => {
                    layout.push(format!(
                        "dojo::database::introspect::FieldLayout {{
                            selector: {},
                            layout: {}
                        }}",
                        get_selector_from_name(m.name.as_str()).unwrap(),
                        array_layout
                    ));
                }
            }
        } else if is_tuple(&m.ty) {
            let tuple_layout = build_item_layout_from_type(diagnostics, diagnostic_item, &m.ty);

            let mut tuple_precomputed_size = 0;
            let mut introspected_size = vec![];
            let is_dynamic =
                compute_tuple_size(&m.ty, &mut tuple_precomputed_size, &mut introspected_size);

            if is_dynamic {
                dynamic_size = true;
            } else {
                size_precompute += tuple_precomputed_size;
                size.extend(introspected_size);
            }

            match composite_type {
                CompositeType::Enum => layout.push(tuple_layout),
                CompositeType::Struct => {
                    layout.push(format!(
                        "dojo::database::introspect::FieldLayout {{
                            selector: {},
                            layout: {}
                        }}",
                        get_selector_from_name(m.name.as_str()).unwrap(),
                        tuple_layout
                    ));
                }
            }
        } else if is_unsupported_option_type(&m.ty) {
            diagnostics.push(PluginDiagnostic {
                stable_ptr: diagnostic_item,
                message: "Option<T> cannot be used with tuples. Prefer using a struct.".into(),
                severity: Severity::Error,
            });
        } else {
            // It's a custom type
            if m.key {
                attrs.push("'key'");
            } else {
                size.push(format!("dojo::database::introspect::Introspect::<{}>::size()", m.ty,));

                match composite_type {
                    CompositeType::Enum => {
                        layout.push(format!(
                            "
                            dojo::database::introspect::FieldLayout {{
                                selector: '',
                                layout: dojo::database::introspect::Introspect::<{}>::layout()
                            }}",
                            m.ty
                        ));
                    }
                    CompositeType::Struct => {
                        layout.push(format!(
                            "dojo::database::introspect::FieldLayout {{
                                selector: {},
                                layout: dojo::database::introspect::Introspect::<{}>::layout()
                            }}",
                            get_selector_from_name(m.name.as_str()).unwrap(),
                            m.ty
                        ));
                    }
                }
            }
        }
    });

    let size = if dynamic_size {
        "Option::None".to_string()
    } else {
        if size_precompute > 0 {
            size.push(format!("Option::Some({})", size_precompute));
        }

        match size.len() {
            0 => "Option::None".to_string(),
            1 => size[0].clone(),
            _ => {
                format!(
                    "let sizes : Array<Option<usize>> = array![
                        {}
                    ];

                    if dojo::database::utils::any_none(@sizes) {{
                        return Option::None;
                    }}
                    Option::Some(dojo::database::utils::sum(sizes))
                    ",
                    size.join(",\n")
                )
            }
        }
    };

    let layout = match composite_type {
        CompositeType::Enum => format!(
            "dojo::database::introspect::Layout::Tuple(
                array![
                {}\n
                ].span()
            )
            ",
            layout.join(",\n")
        ),
        CompositeType::Struct => format!(
            "dojo::database::introspect::Layout::Struct(
                array![
                {}\n
                ].span()
            )
            ",
            layout.join(",\n")
        ),
    };

    RewriteNode::interpolate_patched(
        "
impl $name$Introspect<$generics$> of \
         dojo::database::introspect::Introspect<$name$<$generics_types$>> {
    #[inline(always)]
    fn size() -> Option<usize> {
        $size$
    }

    #[inline(always)]
    fn layout() -> dojo::database::introspect::Layout {
        $layout$
    }

    #[inline(always)]
    fn ty() -> dojo::database::introspect::Ty {
        $ty$
    }
}
        ",
        &UnorderedHashMap::from([
            ("name".to_string(), RewriteNode::Text(name)),
            ("generics".to_string(), RewriteNode::Text(generic_impls)),
            ("generics_types".to_string(), RewriteNode::Text(generics.join(", "))),
            ("size".to_string(), RewriteNode::Text(size)),
            ("layout".to_string(), RewriteNode::Text(layout)),
            ("ty".to_string(), RewriteNode::Text(type_ty)),
        ]),
    )
}

/// Generates enum arm type introspect
pub fn handle_enum_arm_type(ty_name: &String, is_primitive: bool) -> (String, String) {
    let serialized = if is_primitive {
        format!(
            "dojo::database::introspect::serialize_member_type(
                @dojo::database::introspect::Ty::Primitive('{}')
            )",
            ty_name
        )
    } else {
        format!(
            "dojo::database::introspect::serialize_member_type(
                @dojo::database::introspect::Introspect::<{}>::ty()
            )",
            ty_name
        )
    };
    (serialized, ty_name.to_string())
}

fn compute_tuple_size(
    item_type: &String,
    precomputed_size: &mut usize,
    introspected_size: &mut Vec<String>,
) -> bool {
    let primitive_sizes = primitive_type_introspection();
    let items = get_tuple_item_types(item_type);

    for item in items {
        if is_array(&item) || is_byte_array(&item) {
            return true;
        } else if is_tuple(&item) {
            let is_dynamic = compute_tuple_size(&item, precomputed_size, introspected_size);
            if is_dynamic {
                return true;
            }
        } else if let Some(primitive_ty) = primitive_sizes.get(&item) {
            *precomputed_size += primitive_ty.0;
        } else {
            introspected_size
                .push(format!("dojo::database::introspect::Introspect::<{}>::size()", item));
        }
    }

    return false;
}

#[test]
fn test_get_tuple_item_types() {
    fn assert_array(got: Vec<String>, expected: Vec<String>) {
        fn format_array(arr: Vec<String>) -> String {
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
        ("(Array<(Points, Damage)>, ((u16,),)))", vec!["Array<(Points,Damage)>", "((u16,),))"]),
        (
            "(u8, (u16, (u8, u16), Array<(Points, Damage)>), ((u16,),)))",
            vec!["u8", "(u16,(u8,u16),Array<(Points,Damage)>)", "((u16,),))"],
        ),
    ];

    for (value, expected) in test_cases {
        assert_array(
            get_tuple_item_types(&value.to_string()),
            expected.iter().map(|x| x.to_string()).collect::<Vec<_>>(),
        )
    }
}

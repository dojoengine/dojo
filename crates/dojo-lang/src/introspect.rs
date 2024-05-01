use std::collections::HashMap;

use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::{
    Expr, GenericParam, ItemEnum, ItemStruct, OptionTypeClause, OptionWrappedGenericParamList,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_world::manifest::Member;
use itertools::Itertools;

#[derive(PartialEq)]
enum CompositeType {
    Enum,
    Struct,
}

#[derive(Clone, Default)]
struct TypeIntrospection(usize, Vec<usize>);

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

fn is_byte_array(ty: String) -> bool {
    ty.eq("ByteArray")
}

fn is_array_type(ty: String) -> bool {
    ty.starts_with("Array<") || ty.starts_with("Span<")
}

fn get_array_item_type(ty: String) -> String {
    // ByteArray case is handled separately.
    if ty.starts_with("Array<") {
        ty.replace("Array<", "").strip_suffix('>').unwrap().to_string()
    } else {
        ty.replace("Span<", "").strip_suffix('>').unwrap().to_string()
    }
}

/// A handler for Dojo code derives Introspect for a struct
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the struct.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_introspect_struct(db: &dyn SyntaxGroup, struct_ast: ItemStruct) -> RewriteNode {
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
            } else if is_byte_array(ty.clone()) {
                member_types.push(format!(
                    "dojo::database::introspect::serialize_member(
                        @dojo::database::introspect::Member {{
                            name: '{member_name}',
                            ty: dojo::database::introspect::Ty::ByteArray,
                            attrs: array![{}].span()
                        }}
                    )",
                    attrs.join(","),
                ));
            } else if is_array_type(ty.clone()) {
                let item_type = get_array_item_type(ty);

                member_types.push(format!(
                    "dojo::database::introspect::serialize_member(
                        @dojo::database::introspect::Member {{
                        name: '{member_name}',
                        ty: dojo::database::introspect::Ty::Array(
                            dojo::database::introspect::serialize_member_type(
                                @dojo::database::introspect::Introspect::<{item_type}>::ty()
                            )
                        ),
                        attrs: array![{}].span()
                    }})",
                    attrs.join(","),
                ));

                ty = format!("dyn_array__{}", item_type);
            } else if let Expr::Tuple(tuple) = member.type_clause(db).ty(db) {
                let tuple_items = (*tuple.expressions(db))
                    .elements(db)
                    .iter()
                    .map(|e| {
                        let e_ty = e.as_syntax_node().get_text(db).trim().to_string();
                        format!(
                            "dojo::database::introspect::serialize_member_type(
                                @dojo::database::introspect::Introspect::<{e_ty}>::ty()
                            )"
                        )
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

/// A handler for Dojo code derives Introspect for an enum
/// Parameters:
/// * db: The semantic database.
/// * enum_ast: The AST of the enum.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_introspect_enum(
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

            if is_array_type(ty_name.clone()) {
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
) -> RewriteNode {
    let mut size = vec![];
    let mut dynamic_size = false;
    let primitive_sizes = primitive_type_introspection();
    let mut layout = match composite_type {
        CompositeType::Enum => {
            vec![
                "dojo::database::introspect::FieldLayout {
                    selector: '',
                    layout: dojo::database::introspect::Introspect::<u8>::layout()
                }"
                .to_string(),
            ]
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
                                selector: selector!(\"{}\"),
                                layout: \
                             dojo::database::introspect::Layout::Fixed(array![{}].span())
                            }}",
                            m.name, values
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
                            selector: selector!(\"{}\"),
                            layout: dojo::database::introspect::Layout::ByteArray
                        }}",
                        m.name
                    ));
                }
            }
        } else if m.ty.starts_with("dyn_array__") {
            let item_type = m.ty.strip_prefix("dyn_array__").unwrap();
            dynamic_size = true;

            match composite_type {
                CompositeType::Enum => {
                    layout.push(format!(
                        "dojo::database::introspect::Layout::Array(
                            array![
                                dojo::database::introspect::FieldLayout {{
                                    selector: '',
                                    layout: dojo::database::introspect::Introspect::<{}>::layout()
                                }}
                            ].span()
                        )",
                        item_type
                    ));
                }
                CompositeType::Struct => {
                    layout.push(format!(
                        "dojo::database::introspect::FieldLayout {{
                            selector: selector!(\"{}\"),
                            layout: dojo::database::introspect::Layout::Array(
                                array![
                                    dojo::database::introspect::FieldLayout {{
                                        selector: '',
                                        layout: \
                         dojo::database::introspect::Introspect::<{}>::layout()
                                    }}
                                ].span()
                            )
                        }}",
                        m.name, item_type
                    ));
                }
            }
        } else if m.ty.starts_with('(') {
            // this tuple contains primitive types only (checked before)
            // so we can just split the tuple using the coma separator
            let tuple_types =
                m.ty.strip_prefix('(')
                    .unwrap()
                    .strip_suffix(')')
                    .unwrap()
                    .split(',')
                    .collect::<Vec<_>>();

            let tuple_types = tuple_types
                .iter()
                .map(|t| {
                    let type_name = t.trim().to_string();

                    if let Some(p_ty) = primitive_sizes.get(&type_name) {
                        size_precompute += p_ty.0;
                    } else {
                        size.push(format!(
                            "dojo::database::introspect::Introspect::<{}>::size()",
                            type_name
                        ));
                    }

                    format!(
                        "dojo::database::introspect::FieldLayout {{
                            selector: '',
                            layout: dojo::database::introspect::Introspect::<{type_name}>::layout()
                        }}
                        "
                    )
                })
                .collect::<Vec<_>>();

            match composite_type {
                CompositeType::Enum => layout.push(format!(
                    "dojo::database::introspect::Layout::Tuple(
                            array![
                            {}
                            ].span()
                        )",
                    tuple_types.join(",\n")
                )),
                CompositeType::Struct => {
                    layout.push(format!(
                        "dojo::database::introspect::FieldLayout {{
                            selector: selector!(\"{}\"),
                            layout: dojo::database::introspect::Layout::Tuple(
                                array![
                                {}
                                ].span()
                            )
                        }}",
                        m.name,
                        tuple_types.join(",\n")
                    ));
                }
            }
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
                                selector: selector!(\"{}\"),
                                layout: dojo::database::introspect::Introspect::<{}>::layout()
                            }}",
                            m.name, m.ty
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
            0 => {
                // TODO: to verify
                "Option::None".to_string()
            }
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

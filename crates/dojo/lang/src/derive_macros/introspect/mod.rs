use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::{
    GenericParam, ItemEnum, ItemStruct, OptionWrappedGenericParamList,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use itertools::Itertools;

use crate::debug_store_expand;

mod dojo_store;
mod layout;
mod size;
mod ty;
mod utils;

/// Generate the introspect of a Struct
pub fn handle_introspect_struct(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    struct_ast: ItemStruct,
    packed: bool,
) -> RewriteNode {
    let struct_name = struct_ast.name(db).text(db).to_string();

    let gen_types = build_generic_types(db, struct_ast.generic_params(db));

    let inspect_gen_impls =
        build_generic_impls(&gen_types, &["+dojo::meta::introspect::Introspect".to_string()], &[]);
    let dojo_store_gen_impls =
        build_generic_impls(&gen_types, &["+dojo::storage::DojoStore".to_string()], &[]);

    let struct_size = size::compute_struct_layout_size(db, &struct_ast, packed);
    let ty = ty::build_struct_ty(db, &struct_name, &struct_ast);

    let layout = if packed {
        layout::build_packed_struct_layout(db, diagnostics, &struct_ast)
    } else {
        format!(
            "dojo::meta::Layout::Struct(
            array![
            {}
            ].span()
        )",
            layout::build_field_layouts(db, diagnostics, &struct_ast)
        )
    };

    let dojo_store = dojo_store::build_struct_dojo_store(
        db,
        &struct_name,
        &struct_ast,
        &gen_types,
        &dojo_store_gen_impls,
    );

    debug_store_expand(&format!("DOJO_STORE STRUCT::{struct_name}"), &dojo_store);

    generate_introspect(
        &struct_name,
        &struct_size,
        &gen_types,
        inspect_gen_impls,
        &layout,
        &ty,
        &dojo_store,
    )
}

/// Generate the introspect of a Enum
pub fn handle_introspect_enum(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    enum_ast: ItemEnum,
    packed: bool,
) -> RewriteNode {
    let enum_name = enum_ast.name(db).text(db).into();

    let gen_types = build_generic_types(db, enum_ast.generic_params(db));
    let gen_joined_types = gen_types.join(", ");

    let enum_name_with_generics = format!("{enum_name}<{gen_joined_types}>");

    let inspect_gen_impls =
        build_generic_impls(&gen_types, &["+dojo::meta::introspect::Introspect".to_string()], &[]);
    let dojo_store_gen_impls = build_generic_impls(
        &gen_types,
        &["+dojo::storage::DojoStore".to_string(), "+core::serde::Serde".to_string()],
        &[format!("+core::traits::Default<{enum_name_with_generics}>")],
    );

    let variant_sizes = size::compute_enum_variant_sizes(db, &enum_ast);

    let layout = if packed {
        if size::is_enum_packable(&variant_sizes) {
            layout::build_packed_enum_layout(db, diagnostics, &enum_ast)
        } else {
            diagnostics.push(PluginDiagnostic {
                stable_ptr: enum_ast.name(db).stable_ptr().0,
                message: "To be packed, all variants must have fixed layout of same size."
                    .to_string(),
                severity: Severity::Error,
            });
            "ERROR".to_string()
        }
    } else {
        format!(
            "dojo::meta::Layout::Enum(
            array![
            {}
            ].span()
        )",
            layout::build_variant_layouts(db, diagnostics, &enum_ast)
        )
    };

    let enum_size = size::compute_enum_layout_size(&variant_sizes, packed);
    let ty = ty::build_enum_ty(db, &enum_name, &enum_ast);
    let dojo_store = dojo_store::build_enum_dojo_store(
        db,
        &enum_name,
        &enum_ast,
        &gen_types,
        &dojo_store_gen_impls,
    );

    debug_store_expand(&format!("DOJO_STORE ENUM::{enum_name}"), &dojo_store);

    generate_introspect(
        &enum_name,
        &enum_size,
        &gen_types,
        inspect_gen_impls,
        &layout,
        &ty,
        &dojo_store,
    )
}

/// Generate the introspect impl for a Struct or an Enum,
/// based on its name, size, layout and Ty.
fn generate_introspect(
    name: &String,
    size: &String,
    generic_types: &[String],
    generic_impls: String,
    layout: &String,
    ty: &String,
    dojo_store: &String,
) -> RewriteNode {
    RewriteNode::interpolate_patched(
        "
impl $name$Introspect<$generics$> of dojo::meta::introspect::Introspect<$name$<$generics_types$>> \
         {
    #[inline(always)]
    fn size() -> Option<usize> {
        $size$
    }

    #[inline(always)]
    fn layout() -> dojo::meta::Layout {
        $layout$
    }

    #[inline(always)]
    fn ty() -> dojo::meta::introspect::Ty {
        $ty$
    }
}

$dojo_store$
        ",
        &UnorderedHashMap::from([
            ("name".to_string(), RewriteNode::Text(name.to_string())),
            ("generics".to_string(), RewriteNode::Text(generic_impls)),
            ("generics_types".to_string(), RewriteNode::Text(generic_types.join(", "))),
            ("size".to_string(), RewriteNode::Text(size.to_string())),
            ("layout".to_string(), RewriteNode::Text(layout.to_string())),
            ("ty".to_string(), RewriteNode::Text(ty.to_string())),
            ("dojo_store".to_string(), RewriteNode::Text(dojo_store.to_string())),
        ]),
    )
}

// Extract generic type information and build the
// type and impl information to add to the generated introspect
fn build_generic_types(
    db: &dyn SyntaxGroup,
    generic_params: OptionWrappedGenericParamList,
) -> Vec<String> {
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

    generic_types
}

fn build_generic_impls(
    gen_types: &[String],
    base_impls: &[String],
    additional_impls: &[String],
) -> String {
    let mut gen_impls = gen_types
        .iter()
        .map(|g| {
            format!(
                "{g}, {base_impls}",
                base_impls = base_impls.iter().map(|i| format!("{i}<{g}>")).join(", ")
            )
        })
        .collect::<Vec<_>>();

    if !gen_types.is_empty() {
        gen_impls.extend(additional_impls.to_vec());
    }

    gen_impls.join(", ")
}

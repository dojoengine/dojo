use cairo_lang_syntax::node::ast::{
    Expr, ItemEnum, ItemStruct, Member, OptionTypeClause, TypeClause, Variant,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};

use super::utils::{get_array_item_type, get_tuple_item_types, is_array, is_byte_array, is_tuple};

pub fn build_struct_ty(db: &dyn SyntaxGroup, name: &String, struct_ast: &ItemStruct) -> String {
    let members_ty = struct_ast
        .members(db)
        .elements(db)
        .iter()
        .map(|m| build_member_ty(db, m))
        .collect::<Vec<_>>();

    format!(
        "dojo::meta::introspect::Ty::Struct(
            dojo::meta::introspect::Struct {{
                name: '{name}',
                attrs: array![].span(),
                children: array![
                {}\n
                ].span()
            }}
        )",
        members_ty.join(",\n")
    )
}

pub fn build_enum_ty(db: &dyn SyntaxGroup, name: &String, enum_ast: &ItemEnum) -> String {
    let variants = enum_ast.variants(db).elements(db);

    let variants_ty = if variants.is_empty() {
        "".to_string()
    } else {
        variants
            .iter()
            .map(|v| build_variant_ty(db, v))
            .collect::<Vec<_>>()
            .join(",\n")
    };

    format!(
        "dojo::meta::introspect::Ty::Enum(
            dojo::meta::introspect::Enum {{
                name: '{name}',
                attrs: array![].span(),
                children: array![
                {variants_ty}\n
                ].span()
            }}
        )"
    )
}

pub fn build_member_ty(db: &dyn SyntaxGroup, member: &Member) -> String {
    let name = member.name(db).text(db).to_string();
    let attrs = if member.has_attr(db, "key") {
        vec!["'key'"]
    } else {
        vec![]
    };

    format!(
        "dojo::meta::introspect::Member {{
            name: '{name}',
            attrs: array![{}].span(),
            ty: {}
        }}",
        attrs.join(","),
        build_ty_from_type_clause(db, &member.type_clause(db))
    )
}

pub fn build_variant_ty(db: &dyn SyntaxGroup, variant: &Variant) -> String {
    let name = variant.name(db).text(db).to_string();
    match variant.type_clause(db) {
        OptionTypeClause::Empty(_) => {
            // use an empty tuple if the variant has no data
            format!("('{name}', dojo::meta::introspect::Ty::Tuple(array![].span()))")
        }
        OptionTypeClause::TypeClause(type_clause) => {
            format!(
                "('{name}', {})",
                build_ty_from_type_clause(db, &type_clause)
            )
        }
    }
}

pub fn build_ty_from_type_clause(db: &dyn SyntaxGroup, type_clause: &TypeClause) -> String {
    match type_clause.ty(db) {
        Expr::Path(path) => {
            let path_type = path.as_syntax_node().get_text(db).trim().to_string();
            build_item_ty_from_type(&path_type)
        }
        Expr::Tuple(expr) => {
            let tuple_type = expr.as_syntax_node().get_text(db).trim().to_string();
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

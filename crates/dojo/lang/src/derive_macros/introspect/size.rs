use cairo_lang_syntax::node::ast::{Expr, ItemEnum, ItemStruct, OptionTypeClause, TypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::TypedSyntaxNode;

use super::utils::{get_tuple_item_types, is_array, is_byte_array, is_tuple};

pub fn compute_struct_layout_size(
    db: &dyn SyntaxGroup,
    struct_ast: &ItemStruct,
    is_packed: bool,
) -> String {
    let mut cumulated_sizes = 0;
    let mut is_dynamic_size = false;

    let mut sizes = struct_ast
        .members(db)
        .elements(db)
        .into_iter()
        .filter_map(|m| {
            if m.has_attr(db, "key") {
                return None;
            }

            let (sizes, cumulated, is_dynamic) =
                get_field_size_from_type_clause(db, &m.type_clause(db));

            cumulated_sizes += cumulated;
            is_dynamic_size |= is_dynamic;
            Some(sizes)
        })
        .flatten()
        .collect::<Vec<_>>();
    build_size_function_body(&mut sizes, cumulated_sizes, is_dynamic_size, is_packed)
}

pub fn compute_enum_variant_sizes(
    db: &dyn SyntaxGroup,
    enum_ast: &ItemEnum,
) -> Vec<(Vec<String>, u32, bool)> {
    enum_ast
        .variants(db)
        .elements(db)
        .iter()
        .map(|v| match v.type_clause(db) {
            OptionTypeClause::Empty(_) => (vec![], 0, false),
            OptionTypeClause::TypeClause(type_clause) => {
                get_field_size_from_type_clause(db, &type_clause)
            }
        })
        .collect::<Vec<_>>()
}

pub fn is_enum_packable(variant_sizes: &[(Vec<String>, u32, bool)]) -> bool {
    if variant_sizes.is_empty() {
        return true;
    }

    let v0_sizes = variant_sizes[0].0.clone();
    let v0_fixed_size = variant_sizes[0].1;

    variant_sizes.iter().all(|vs| {
        vs.0.len() == v0_sizes.len()
            && vs.0.iter().zip(v0_sizes.iter()).all(|(a, b)| a == b)
            && vs.1 == v0_fixed_size
            && !vs.2
    })
}

pub fn compute_enum_layout_size(
    variant_sizes: &[(Vec<String>, u32, bool)],
    is_packed: bool,
) -> String {
    if variant_sizes.is_empty() {
        return "Option::None".to_string();
    }

    let v0 = variant_sizes[0].clone();
    let identical_variants =
        variant_sizes.iter().all(|vs| vs.0 == v0.0 && vs.1 == v0.1 && vs.2 == v0.2);

    if identical_variants {
        let (mut sizes, mut cumulated_sizes, is_dynamic_size) = v0;

        // add one felt252 to store the variant identifier
        cumulated_sizes += 1;

        build_size_function_body(&mut sizes, cumulated_sizes, is_dynamic_size, is_packed)
    } else {
        "Option::None".to_string()
    }
}

pub fn build_size_function_body(
    sizes: &mut Vec<String>,
    cumulated_sizes: u32,
    is_dynamic_size: bool,
    is_packed: bool,
) -> String {
    if is_dynamic_size {
        return "Option::None".to_string();
    }

    if cumulated_sizes > 0 {
        sizes.push(format!("Option::Some({})", cumulated_sizes));
    }

    match sizes.len() {
        0 => "Option::None".to_string(),
        1 => sizes[0].clone(),
        _ => {
            let none_check = if is_packed {
                ""
            } else {
                "if dojo::utils::any_none(@sizes) {
                    return Option::None;
                }"
            };

            format!(
                "let sizes : Array<Option<usize>> = array![
                    {}
                ];

                {none_check}
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
) -> (Vec<String>, u32, bool) {
    let mut cumulated_sizes = 0;
    let mut is_dynamic_size = false;

    let field_sizes = match type_clause.ty(db) {
        Expr::Path(path) => {
            let path_type = path.as_syntax_node().get_text(db).trim().to_string();
            compute_item_size_from_type(&path_type)
        }
        Expr::Tuple(expr) => {
            let tuple_type = expr.as_syntax_node().get_text(db).trim().to_string();
            compute_tuple_size_from_type(&tuple_type)
        }
        _ => {
            // field type already checked while building the layout
            vec!["ERROR".to_string()]
        }
    };

    let sizes = field_sizes
        .into_iter()
        .filter_map(|s| match s.parse::<u32>() {
            Ok(v) => {
                cumulated_sizes += v;
                None
            }
            Err(_) => {
                if s.eq("Option::None") {
                    is_dynamic_size = true;
                    None
                } else {
                    Some(s)
                }
            }
        })
        .collect::<Vec<_>>();

    (sizes, cumulated_sizes, is_dynamic_size)
}

pub fn compute_item_size_from_type(item_type: &String) -> Vec<String> {
    if is_array(item_type) || is_byte_array(item_type) {
        vec!["Option::None".to_string()]
    } else if is_tuple(item_type) {
        compute_tuple_size_from_type(item_type)
    } else {
        vec![format!("dojo::meta::introspect::Introspect::<{}>::size()", item_type)]
    }
}

pub fn compute_tuple_size_from_type(tuple_type: &str) -> Vec<String> {
    get_tuple_item_types(tuple_type)
        .iter()
        .flat_map(compute_item_size_from_type)
        .collect::<Vec<_>>()
}

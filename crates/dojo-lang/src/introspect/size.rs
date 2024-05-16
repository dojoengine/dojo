use cairo_lang_syntax::node::ast::{Expr, ItemEnum, ItemStruct, OptionTypeClause, TypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::TypedSyntaxNode;

use super::utils::{
    get_tuple_item_types, is_array, is_byte_array, is_tuple, primitive_type_introspection,
};

pub fn compute_struct_layout_size(db: &dyn SyntaxGroup, struct_ast: &ItemStruct) -> String {
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
    build_size_function_body(&mut sizes, cumulated_sizes, is_dynamic_size)
}

pub fn compute_enum_layout_size(
    db: &dyn SyntaxGroup,
    enum_ast: &ItemEnum,
    identical_variants: bool,
) -> String {
    if identical_variants {
        match enum_ast.variants(db).elements(db).first() {
            Some(first_variant) => {
                let (mut sizes, cumulated_sizes, is_dynamic_size) =
                    match first_variant.type_clause(db) {
                        OptionTypeClause::Empty(_) => (vec![], 0, false),
                        OptionTypeClause::TypeClause(type_clause) => {
                            get_field_size_from_type_clause(db, &type_clause)
                        }
                    };

                // add 8 bits to store the variant identifier
                sizes.insert(0, "8".to_string());

                build_size_function_body(&mut sizes, cumulated_sizes, is_dynamic_size)
            }
            None => return "Option::None".to_string(),
        }
    } else {
        "Option::None".to_string()
    }
}

pub fn build_size_function_body(
    sizes: &mut Vec<String>,
    cumulated_sizes: u32,
    is_dynamic_size: bool,
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
            format!(
                "let sizes : Array<Option<usize>> = array![
                        {}
                    ];

                    if dojo::database::utils::any_none(@sizes) {{
                        return Option::None;
                    }}
                    Option::Some(dojo::database::utils::sum(sizes))
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
        let primitives = primitive_type_introspection();

        if let Some(p) = primitives.get(item_type) {
            vec![p.0.to_string()]
        } else {
            vec![format!("dojo::database::introspect::Introspect::<{}>::size()", item_type)]
        }
    }
}

pub fn compute_tuple_size_from_type(tuple_type: &String) -> Vec<String> {
    get_tuple_item_types(tuple_type)
        .iter()
        .map(|x| compute_item_size_from_type(x))
        .flatten()
        .collect::<Vec<_>>()
}

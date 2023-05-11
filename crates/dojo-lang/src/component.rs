use std::collections::HashMap;

use cairo_lang_defs::plugin::{DynGeneratedFileAuxData, PluginGeneratedFile, PluginResult};
use cairo_lang_semantic::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_semantic::plugin::DynPluginAuxData;
use cairo_lang_syntax::attribute::structured::{
    AttributeArg, AttributeArgVariant, AttributeStructurize,
};
use cairo_lang_syntax::node::ast::ItemStruct;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};

use crate::plugin::DojoAuxData;

pub fn handle_component_struct(db: &dyn SyntaxGroup, struct_ast: ItemStruct) -> PluginResult {
    let mut body_nodes = vec![RewriteNode::interpolate_patched(
        "
            #[view]
            fn name() -> felt252 {
                '$type_name$'
            }

            #[view]
            fn len() -> usize {
                $len$_usize
            }
        ",
        HashMap::from([
            (
                "type_name".to_string(),
                RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node()),
            ),
            ("members".to_string(), RewriteNode::Copied(struct_ast.members(db).as_syntax_node())),
            (
                "len".to_string(),
                RewriteNode::Text(struct_ast.members(db).elements(db).len().to_string()),
            ),
        ]),
    )];

    let is_indexed_fn = {
        let retval_str = if is_indexed(db, struct_ast.clone()) {
            "True".to_string()
        } else {
            "False".to_string()
        };

        RewriteNode::interpolate_patched(
            "
                #[view]
                fn is_indexed() -> bool {
                    bool::$retval$(())
                }
            ",
            HashMap::from([("retval".to_string(), RewriteNode::Text(retval_str))]),
        )
    };

    // Add the is_indexed function to the body
    body_nodes.push(is_indexed_fn);

    let mut schema = vec![];

    let binding = struct_ast.members(db).elements(db);
    binding.iter().for_each(|member| {
        schema.push(RewriteNode::interpolate_patched(
            "array::ArrayTrait::append(ref arr, ('$name$' , '$type_clause$' , 252));\n",
            HashMap::from([
                ("name".to_string(), RewriteNode::new_trimmed(member.name(db).as_syntax_node())),
                (
                    "type_clause".to_string(),
                    RewriteNode::new_trimmed(member.type_clause(db).ty(db).as_syntax_node()),
                ),
            ]),
        ));
    });

    let name = struct_ast.name(db).text(db);
    let mut builder = PatchBuilder::new(db);
    let derive = {
        if is_custom(db, struct_ast.clone()) {
            // Get the traits that the user wants to derive
            format!("({})", get_traits(db, struct_ast.clone()).join(", "))
        } else {
            "(Copy, Drop, Serde)".to_string()
        }
    };

    builder.add_modified(RewriteNode::interpolate_patched(
        "
            #[derive$derive$]
            struct $type_name$ {
                $members$
            }

            #[abi]
            trait I$type_name$ {
                fn name() -> felt252;
                fn len() -> u8;
            }

            #[contract]
            mod $type_name$Component {
                use dojo_core::serde::SpanSerde;
                use super::$type_name$;

                #[view]
                fn schema() -> Array<(felt252, felt252, u8)> {
                    let mut arr = array::ArrayTrait::new();
                    $schemas$
                    arr
                }

                $body$
            }
        ",
        HashMap::from([
            ("derive".to_string(), RewriteNode::Text(derive)),
            (
                "type_name".to_string(),
                RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node()),
            ),
            ("members".to_string(), RewriteNode::Copied(struct_ast.members(db).as_syntax_node())),
            ("body".to_string(), RewriteNode::new_modified(body_nodes)),
            ("schemas".to_string(), RewriteNode::new_modified(schema)),
        ]),
    ));

    PluginResult {
        code: Some(PluginGeneratedFile {
            name: name.clone(),
            content: builder.code,
            aux_data: DynGeneratedFileAuxData::new(DynPluginAuxData::new(DojoAuxData {
                patches: builder.patches,
                components: vec![name],
                systems: vec![],
            })),
        }),
        diagnostics: vec![],
        remove_original_item: true,
    }
}

/// Returns true if the component is indexed #[component(indexed: true)]
fn is_indexed(db: &dyn SyntaxGroup, struct_ast: ItemStruct) -> bool {
    for attr in struct_ast.attributes(db).query_attr(db, "component") {
        let attr = attr.structurize(db);

        for arg in attr.args {
            let AttributeArg {
                variant: AttributeArgVariant::Named {
                    value: ast::Expr::True(_),
                    name,
                    ..
                },
                ..
            } = arg else {
                continue;
            };

            if name == "indexed" {
                return true;
            }
        }
    }
    false
}

/// Returns true if the component is custom #[component(custom: true)]
fn is_custom(db: &dyn SyntaxGroup, struct_ast: ItemStruct) -> bool {
    for attr in struct_ast.attributes(db).query_attr(db, "component") {
        let attr = attr.structurize(db);

        for arg in attr.args {
            let AttributeArg {
                variant: AttributeArgVariant::Named {
                    value,
                    name,
                    ..
                },
                ..
            } = arg else {
                continue;
            };

            if name == "custom" {
                if let Expr::True(_) = value {
                    return true;
                }
            }
        }
    }
    false
}

/// Returns all the traits defined in the component attribute
/// #[component(indexed: true, custom: true, traits: (Copy, Drop))]
/// Returns ["Copy", "Drop"]
fn get_traits(db: &dyn SyntaxGroup, struct_ast: ItemStruct) -> Vec<String> {
    let mut traits = Vec::new();

    for attr in struct_ast.attributes(db).query_attr(db, "component") {
        let attr = attr.structurize(db);

        for arg in attr.args {
            // Unpack the attribute argument
            let AttributeArg {
                variant: AttributeArgVariant::Named {
                    value,
                    name,
                    ..
                },
                ..
            } = arg else {
                continue;
            };

            // Check if the attribute is traits
            if name == "traits" {
                // Check if the value is a tuple i.e. (Copy, Drop)
                if let Expr::Tuple(tuple_expr) = value {
                    // Loop over the elements in the tuple
                    let tuple_iter: Vec<Expr> = tuple_expr.expressions(db).elements(db);
                    for elem in tuple_iter {
                        // Check if the element is an identifier (which it should be for a trait)
                        if let Expr::Path(path_expr) = elem {
                            // Add the trait to the vector
                            traits.push(path_expr.node.get_text(db));
                        }
                    }
                // Check if the value is parenthesized i.e. (Copy)
                } else if let Expr::Parenthesized(parenthesized_expr) = value {
                    // Check if the parenthesized expression is a path expression
                    if let Expr::Path(path_expr) = parenthesized_expr.expr(db) {
                        // Add the trait to the vector
                        traits.push(path_expr.node.get_text(db));
                    }
                }
            }
        }
    }
    traits
}

use std::collections::HashMap;

use cairo_lang_defs::plugin::{DynGeneratedFileAuxData, PluginGeneratedFile, PluginResult};
use cairo_lang_semantic::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_semantic::plugin::DynPluginAuxData;
use cairo_lang_syntax::attribute::structured::{
    AttributeArg, AttributeArgVariant, AttributeStructurize,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};

use crate::plugin::DojoAuxData;

pub fn handle_component_struct(db: &dyn SyntaxGroup, struct_ast: ast::ItemStruct) -> PluginResult {
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

    let mut serialize = vec![];
    let mut deserialize = vec![];
    let mut schema = vec![];

    let binding = struct_ast.members(db).elements(db);
    binding.iter().for_each(|member| {
        schema.push(RewriteNode::interpolate_patched(
            "array.append(('$name$' , '$type_clause$' , 252));\n",
            HashMap::from([
                ("name".to_string(), RewriteNode::new_trimmed(member.name(db).as_syntax_node())),
                (
                    "type_clause".to_string(),
                    RewriteNode::new_trimmed(member.type_clause(db).ty(db).as_syntax_node()),
                ),
            ]),
        ));

        serialize.push(RewriteNode::interpolate_patched(
            "serde::Serde::<$type_clause$>::serialize(ref serialized, input.$key$);",
            HashMap::from([
                ("key".to_string(), RewriteNode::new_trimmed(member.name(db).as_syntax_node())),
                (
                    "type_clause".to_string(),
                    RewriteNode::new_trimmed(member.type_clause(db).ty(db).as_syntax_node()),
                ),
            ]),
        ));

        deserialize.push(RewriteNode::interpolate_patched(
            "$key$: serde::Serde::<$type_clause$>::deserialize(ref serialized)?,",
            HashMap::from([
                ("key".to_string(), RewriteNode::new_trimmed(member.name(db).as_syntax_node())),
                (
                    "type_clause".to_string(),
                    RewriteNode::new_trimmed(member.type_clause(db).ty(db).as_syntax_node()),
                ),
            ]),
        ));
    });

    let name = struct_ast.name(db).text(db);
    let mut builder = PatchBuilder::new(db);
    builder.add_modified(RewriteNode::interpolate_patched(
        "
            #[derive(Copy, Drop, Serde)]
            struct $type_name$ {
                $members$
            }

            #[abi]
            trait I$type_name$ {
                fn name() -> felt252;
                fn len() -> u8;
                fn serialize(raw: Span<felt252>) -> $type_name$;
                fn deserialize(value: $type_name$) -> Span<felt252>;
            }

            #[contract]
            mod $type_name$Component {
                use array::ArrayTrait;
                use option::OptionTrait;
                use dojo_core::serde::SpanSerde;
                use super::$type_name$;

                #[view]
                fn schema() -> Array<(felt252,felt252,u8)> {
                    let mut array=ArrayTrait::new();
                    $schemas$
                    array
                }

                $body$
            }
        ",
        HashMap::from([
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

fn is_indexed(db: &dyn SyntaxGroup, struct_ast: ast::ItemStruct) -> bool {
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

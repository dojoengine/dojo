use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::attribute::structured::{
    AttributeArg, AttributeArgVariant, AttributeStructurize,
};
use cairo_lang_syntax::node::ast::ItemStruct;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use convert_case::{Case, Casing};
use dojo_types::component::Member;

use crate::plugin::{Component, DojoAuxData};

/// A handler for Dojo code that modifies a component struct.
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the component struct.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_component_struct(
    db: &dyn SyntaxGroup,
    aux_data: &mut DojoAuxData,
    struct_ast: ItemStruct,
) -> RewriteNode {
    let mut body_nodes = vec![RewriteNode::interpolate_patched(
        "
            #[external(v0)]
            fn name(self: @ContractState) -> felt252 {
                '$type_name$'
            }

            #[external(v0)]
            fn len(self: @ContractState) -> usize {
                $len$_usize
            }
        ",
        UnorderedHashMap::from([
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
                #[external(v0)]
                fn is_indexed(self: @ContractState) -> bool {
                    bool::$retval$(())
                }
            ",
            UnorderedHashMap::from([("retval".to_string(), RewriteNode::Text(retval_str))]),
        )
    };

    // Add the is_indexed function to the body
    body_nodes.push(is_indexed_fn);

    let members: Vec<_> = struct_ast
        .members(db)
        .elements(db)
        .iter()
        .enumerate()
        .map(|(slot, member)| {
            (member.name(db).text(db), member.type_clause(db).ty(db), slot as u64, 0)
        })
        .collect::<_>();

    let name = struct_ast.name(db).text(db);
    aux_data.components.push(Component {
        name: name.to_string(),
        members: members
            .iter()
            .map(|(name, ty, slot, offset)| Member {
                name: name.to_string(),
                ty: ty.as_syntax_node().get_text(db).trim().to_string(),
                slot: *slot,
                offset: *offset,
            })
            .collect(),
    });

    RewriteNode::interpolate_patched(
        "
            struct $type_name$ {
                $members$
            }

            #[starknet::interface]
            trait I$type_name$<T> {
                fn name(self: @T) -> felt252;
                fn len(self: @T) -> u8;
            }

            #[starknet::contract]
            mod $contract_name$ {
                use super::$type_name$;

                #[storage]
                struct Storage {}

                #[external(v0)]
                fn schema(self: @ContractState) -> Array<(felt252, felt252, usize, u8)> {
                    let mut arr = array::ArrayTrait::new();
                    $schema_members$
                    arr
                }

                $body$
            }
        ",
        UnorderedHashMap::from([
            ("contract_name".to_string(), RewriteNode::Text(name.to_case(Case::Snake))),
            (
                "type_name".to_string(),
                RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node()),
            ),
            ("members".to_string(), RewriteNode::Copied(struct_ast.members(db).as_syntax_node())),
            ("body".to_string(), RewriteNode::new_modified(body_nodes)),
            (
                "schema_members".to_string(),
                RewriteNode::new_modified(
                    members
                        .iter()
                        .map(|item| {
                            RewriteNode::interpolate_patched(
                                "array::ArrayTrait::append(ref arr, ('$name$', '$type_clause$', \
                                 $slot$, $offset$));\n",
                                UnorderedHashMap::from([
                                    ("name".to_string(), RewriteNode::Text(item.0.to_string())),
                                    (
                                        "type_clause".to_string(),
                                        RewriteNode::new_trimmed(item.1.as_syntax_node()),
                                    ),
                                    ("slot".to_string(), RewriteNode::Text(item.2.to_string())),
                                    ("offset".to_string(), RewriteNode::Text(item.3.to_string())),
                                ]),
                            )
                        })
                        .collect(),
                ),
            ),
        ]),
    )
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

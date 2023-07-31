use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::ast::ItemStruct;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
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
) -> (RewriteNode, Vec<PluginDiagnostic>) {
    let mut diagnostics = vec![];

    let members: Vec<_> = struct_ast
        .members(db)
        .elements(db)
        .iter()
        .enumerate()
        .map(|(slot, member)| {
            (member.name(db).text(db), member.type_clause(db).ty(db), slot as u64, 0)
        })
        .collect::<_>();

    let serialized_keys: Vec<_> = struct_ast
        .members(db)
        .elements(db)
        .iter()
        .filter_map(|e| {
            if e.has_attr(db, "key") {
                return Some(RewriteNode::Text(format!(
                    "self.{}.serialize(ref serialized);",
                    e.name(db).text(db)
                )));
            }

            None
        })
        .collect::<_>();

    if serialized_keys.len() == 0 {
        diagnostics.push(PluginDiagnostic {
            message: "Component must define atleast one #[key] attribute".into(),
            stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
        });
    }

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

    (
        RewriteNode::interpolate_patched(
            "
            struct $type_name$ {
                $members$
            }

            impl $type_name$Component of dojo::traits::Component<$type_name$> {
                fn name(self: @$type_name$) -> felt252 {
                    '$type_name$'
                }

                fn key(self: @$type_name$) -> felt252 {
                    let mut serialized = ArrayTrait::new();
                    $serialized_keys$
                    poseidon::poseidon_hash_span(serialized.span())
                }
            }

            #[starknet::interface]
            trait I$type_name$<T> {
                fn name(self: @T) -> felt252;
            }

            #[starknet::contract]
            mod $contract_name$ {
                use super::$type_name$;

                #[storage]
                struct Storage {}

                #[external(v0)]
                fn name(self: @ContractState) -> felt252 {
                    '$type_name$'
                }

                #[external(v0)]
                fn size(self: @ContractState) -> usize {
                    dojo::SerdeLen::<$type_name$>::len()
                }
            }
        ",
            UnorderedHashMap::from([
                ("contract_name".to_string(), RewriteNode::Text(name.to_case(Case::Snake))),
                (
                    "type_name".to_string(),
                    RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node()),
                ),
                (
                    "members".to_string(),
                    RewriteNode::Copied(struct_ast.members(db).as_syntax_node()),
                ),
                ("serialized_keys".to_string(), RewriteNode::new_modified(serialized_keys)),
            ]),
        ),
        diagnostics,
    )
}

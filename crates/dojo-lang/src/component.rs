use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::ast::ItemStruct;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use convert_case::{Case, Casing};
use dojo_types::component::Member;
use itertools::Itertools;

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

    let elements = struct_ast.members(db).elements(db);
    let keys: Vec<_> = elements
        .iter()
        .filter_map(|e| {
            if e.has_attr(db, "key") {
                return Some(e);
            }

            None
        })
        .collect::<_>();

    if keys.len() == 0 {
        diagnostics.push(PluginDiagnostic {
            message: "Component must define atleast one #[key] attribute".into(),
            stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
        });
    }

    let key_names = keys.iter().map(|e| e.name(db).text(db)).join(", ");

    let key_types =
        keys.iter().map(|e| e.type_clause(db).ty(db).as_syntax_node().get_text(db)).join(", ");

    let serialized_keys: Vec<_> = keys
        .iter()
        .map(|e| {
            if e.type_clause(db).ty(db).as_syntax_node().get_text(db) == "felt252" {
                return RewriteNode::Text(format!("serialized.append({});\n", e.name(db).text(db)));
            }

            RewriteNode::Text(format!(
                "serde::Serde::serialize(@{}, ref serialized);\n",
                e.name(db).text(db)
            ))
        })
        .collect::<_>();

    let component_serialized_keys: Vec<_> = keys
        .iter()
        .map(|e| {
            if e.type_clause(db).ty(db).as_syntax_node().get_text(db) == "felt252" {
                return RewriteNode::Text(format!(
                    "serialized.append(*self.{});\n",
                    e.name(db).text(db)
                ));
            }

            RewriteNode::Text(format!(
                "serde::Serde::serialize(self.{}, ref serialized);\n",
                e.name(db).text(db)
            ))
        })
        .collect::<_>();

    let component_serialized_values: Vec<_> = elements
        .iter()
        .filter_map(|e| {
            if !e.has_attr(db, "key") {
                if e.type_clause(db).ty(db).as_syntax_node().get_text(db) == "felt252" {
                    return Some(RewriteNode::Text(format!(
                        "serialized.append(*self.{});\n",
                        e.name(db).text(db)
                    )));
                }

                return Some(RewriteNode::Text(format!(
                    "serde::Serde::serialize(self.{}, ref serialized);",
                    e.name(db).text(db)
                )));
            }

            None
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

    (
        RewriteNode::interpolate_patched(
            "
            struct $type_name$ {
                $members$
            }

            trait $type_name$KeysTrait {
                fn serialize_keys(keys: ($key_types$, )) -> Span<felt252>;
            }

            impl $type_name$KeysImpl of $type_name$KeysTrait {
                #[inline(always)]
                fn serialize_keys(keys: ($key_types$, )) -> Span<felt252> {
                    let ($key_names$, ): ($key_types$, ) = keys;
                    let mut serialized = ArrayTrait::new();
                    $serialized_keys$
                    serialized.span()
                }
            }

            impl $type_name$Component of dojo::traits::Component<$type_name$> {
                #[inline(always)]
                fn name(self: @$type_name$) -> felt252 {
                    '$type_name$'
                }

                #[inline(always)]
                fn keys(self: @$type_name$) -> Span<felt252> {
                    let mut serialized = ArrayTrait::new();
                    $component_serialized_keys$
                    serialized.span()
                }

                #[inline(always)]
                fn values(self: @$type_name$) -> Span<felt252> {
                    let mut serialized = ArrayTrait::new();
                    $component_serialized_values$
                    serialized.span()
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
                ("key_names".to_string(), RewriteNode::Text(key_names)),
                ("key_types".to_string(), RewriteNode::Text(key_types)),
                ("serialized_keys".to_string(), RewriteNode::new_modified(serialized_keys)),
                (
                    "component_serialized_keys".to_string(),
                    RewriteNode::new_modified(component_serialized_keys),
                ),
                (
                    "component_serialized_values".to_string(),
                    RewriteNode::new_modified(component_serialized_values),
                ),
            ]),
        ),
        diagnostics,
    )
}

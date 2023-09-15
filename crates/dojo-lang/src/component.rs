use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
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

    let elements = struct_ast.members(db).elements(db);
    let members: &Vec<_> = &elements
        .iter()
        .map(|member| Member {
            name: member.name(db).text(db).to_string(),
            ty: member.type_clause(db).ty(db).as_syntax_node().get_text(db).trim().to_string(),
            key: member.has_attr(db, "key"),
        })
        .collect::<_>();

    let keys: Vec<_> = members.iter().filter(|m| m.key).collect::<_>();

    if keys.is_empty() {
        diagnostics.push(PluginDiagnostic {
            message: "Component must define atleast one #[key] attribute".into(),
            stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
        });
    }

    let serialize_member = |m: &Member, include_key: bool| {
        if m.key && !include_key {
            return None;
        }

        if m.ty == "felt252" {
            return Some(RewriteNode::Text(format!(
                "array::ArrayTrait::append(ref serialized, *self.{});\n",
                m.name
            )));
        }

        Some(RewriteNode::Text(format!(
            "serde::Serde::serialize(self.{}, ref serialized);",
            m.name
        )))
    };

    let serialized_keys: Vec<_> =
        keys.iter().filter_map(|m| serialize_member(m, true)).collect::<_>();

    let serialized_values: Vec<_> =
        members.iter().filter_map(|m| serialize_member(m, false)).collect::<_>();

    let schema = members
        .iter()
        .map(|m| {
            RewriteNode::interpolate_patched(
                "array::ArrayTrait::append(ref arr, ('$name$', '$typ$', $is_key$));",
                UnorderedHashMap::from([
                    ("name".to_string(), RewriteNode::Text(m.name.to_string())),
                    ("typ".to_string(), RewriteNode::Text(m.ty.to_string())),
                    ("is_key".to_string(), RewriteNode::Text(m.key.to_string())),
                ]),
            )
        })
        .collect::<_>();

    let name = struct_ast.name(db).text(db);
    aux_data.components.push(Component { name: name.to_string(), members: members.to_vec() });

    let prints: Vec<_> = members
        .iter()
        .map(|m| {
            format!(
                "debug::PrintTrait::print('{}'); debug::PrintTrait::print(self.{});",
                m.name, m.name
            )
        })
        .collect();

    (
        RewriteNode::interpolate_patched(
            "
            struct $type_name$ {
                $members$
            }

            impl $type_name$Component of dojo::traits::Component<$type_name$> {
                #[inline(always)]
                fn name(self: @$type_name$) -> felt252 {
                    '$type_name$'
                }

                #[inline(always)]
                fn keys(self: @$type_name$) -> Span<felt252> {
                    let mut serialized = ArrayTrait::new();
                    $serialized_keys$
                    array::ArrayTrait::span(@serialized)
                }

                #[inline(always)]
                fn values(self: @$type_name$) -> Span<felt252> {
                    let mut serialized = ArrayTrait::new();
                    $serialized_values$
                    array::ArrayTrait::span(@serialized)
                }
            }

            #[cfg(test)]
            impl $type_name$PrintImpl of debug::PrintTrait<$type_name$> {
                fn print(self: $type_name$) {
                    $print$
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

                #[external(v0)]
                fn schema(self: @ContractState) -> Array<(felt252, felt252, bool)> {
                    let mut arr = array::ArrayTrait::new();
                    $schema$
                    arr
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
                (
                    "key_names".to_string(),
                    RewriteNode::Text(keys.iter().map(|m| m.name.to_string()).join(", ")),
                ),
                (
                    "key_types".to_string(),
                    RewriteNode::Text(keys.iter().map(|m| m.ty.to_string()).join(", ")),
                ),
                ("serialized_keys".to_string(), RewriteNode::new_modified(serialized_keys)),
                ("serialized_values".to_string(), RewriteNode::new_modified(serialized_values)),
                ("schema".to_string(), RewriteNode::new_modified(schema)),
                ("print".to_string(), RewriteNode::Text(prints.join("\n"))),
            ]),
        ),
        diagnostics,
    )
}

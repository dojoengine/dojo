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

    let members: Vec<_> = struct_ast
        .members(db)
        .elements(db)
        .iter()
        .map(|member| {
            (member.name(db).text(db), member.type_clause(db).ty(db), member.has_attr(db, "key"))
        })
        .collect::<_>();

    let elements = struct_ast.members(db).elements(db);
    let keys: Vec<_> = elements.iter().filter(|e| e.has_attr(db, "key")).collect::<_>();

    if keys.is_empty() {
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
                return RewriteNode::Text(format!(
                    "array::ArrayTrait::append(ref serialized, {});\n",
                    e.name(db).text(db)
                ));
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
                    "array::ArrayTrait::append(ref serialized, *self.{});\n",
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
                        "array::ArrayTrait::append(ref serialized, *self.{});\n",
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

    let schema = elements
        .iter()
        .map(|member| {
            RewriteNode::interpolate_patched(
                "array::ArrayTrait::append(ref arr, ('$name$', '$typ$', $is_key$));",
                UnorderedHashMap::from([
                    (
                        "name".to_string(),
                        RewriteNode::new_trimmed(member.name(db).as_syntax_node()),
                    ),
                    (
                        "typ".to_string(),
                        RewriteNode::new_trimmed(member.type_clause(db).ty(db).as_syntax_node()),
                    ),
                    (
                        "is_key".to_string(),
                        RewriteNode::Text(member.has_attr(db, "key").to_string()),
                    ),
                ]),
            )
        })
        .collect::<_>();

    let name = struct_ast.name(db).text(db);
    aux_data.components.push(Component {
        name: name.to_string(),
        members: members
            .iter()
            .map(|(name, ty, key)| Member {
                name: name.to_string(),
                ty: ty.as_syntax_node().get_text(db).trim().to_string(),
                key: *key,
            })
            .collect(),
    });

    let member_prints: Vec<_> = members
        .iter()
        .map(|member| {
            let member_name = &member.0;
            format!(
                "debug::PrintTrait::print('{}'); debug::PrintTrait::print(self.{});",
                member_name, member_name
            )
        })
        .collect();

    let print_body = member_prints.join("\n");

    (
        RewriteNode::interpolate_patched(
            "
            struct $type_name$ {
                $members$
            }

            impl $type_name$Component of dojo::component::Component<$type_name$> {
                #[inline(always)]
                fn name(self: @$type_name$) -> felt252 {
                    '$type_name$'
                }

                #[inline(always)]
                fn keys(self: @$type_name$) -> Span<felt252> {
                    let mut serialized = ArrayTrait::new();
                    $component_serialized_keys$
                    array::ArrayTrait::span(@serialized)
                }

                #[inline(always)]
                fn values(self: @$type_name$) -> Span<felt252> {
                    let mut serialized = ArrayTrait::new();
                    $component_serialized_values$
                    array::ArrayTrait::span(@serialized)
                }
            }

            impl $type_name$StorageSize of dojo::StorageSize<$type_name$> {
                #[inline(always)]
                fn unpacked_size() -> usize {
                    $unpacked_size$
                }

                #[inline(always)]
                fn packed_size() -> usize {
                    $packed_size$
                }
            }

            #[cfg(test)]
            impl $type_name$PrintImpl of debug::PrintTrait<$type_name$> {
                fn print(self: $type_name$) {
                    $print_body$
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
                    dojo::StorageSize::<$type_name$>::unpacked_size()
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
                ("schema".to_string(), RewriteNode::new_modified(schema)),
                ("print_body".to_string(), RewriteNode::Text(print_body)),
                (
                    "unpacked_size".to_string(),
                    RewriteNode::Text(
                        struct_ast
                            .members(db)
                            .elements(db)
                            .iter()
                            .filter_map(|member| {
                                if member.has_attr(db, "key") {
                                    return None;
                                }

                                Some(format!(
                                    "dojo::StorageSize::<{}>::unpacked_size()",
                                    member.type_clause(db).ty(db).as_syntax_node().get_text(db),
                                ))
                            })
                            .join(" + "),
                    ),
                ),
                (
                    "packed_size".to_string(),
                    RewriteNode::Text(
                        struct_ast
                            .members(db)
                            .elements(db)
                            .iter()
                            .filter_map(|member| {
                                if member.has_attr(db, "key") {
                                    return None;
                                }

                                Some(format!(
                                    "dojo::StorageSize::<{}>::packed_size()",
                                    member.type_clause(db).ty(db).as_syntax_node().get_text(db),
                                ))
                            })
                            .join(" + "),
                    ),
                ),
            ]),
        ),
        diagnostics,
    )
}

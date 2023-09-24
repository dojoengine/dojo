use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_syntax::node::ast::ItemStruct;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use convert_case::{Case, Casing};
use dojo_world::manifest::Member;
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
                "array::ArrayTrait::append(ref serialized, *self.{});",
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

    let layout: Vec<_> = members
        .iter()
        .filter_map(|m| {
            if m.key {
                return None;
            }

            Some(RewriteNode::Text(format!(
                "dojo::database::schema::SchemaIntrospection::<{}>::layout(ref layout);\n",
                m.ty
            )))
        })
        .collect::<_>();

    let member_types: Vec<_> = members
        .iter()
        .map(|m| {
            let mut attrs = vec![];
            if m.key {
                attrs.push("'key'")
            }

            format!(
                "dojo::database::schema::serialize_member(@dojo::database::schema::Member {{
                    name: '{}',
                    ty: dojo::database::schema::SchemaIntrospection::<{}>::ty(),
                    attrs: array![{}].span()
                }})",
                m.name,
                m.ty,
                attrs.join(","),
            )
        })
        .collect::<_>();

    let name = struct_ast.name(db).text(db);
    aux_data.components.push(Component { name: name.to_string(), members: members.to_vec() });

    (
        RewriteNode::interpolate_patched(
            "
            impl $type_name$Component of dojo::component::Component<$type_name$> {
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

                #[inline(always)]
                fn layout(self: @$type_name$) -> Span<u8> {
                    let mut layout = ArrayTrait::new();
                    dojo::database::schema::SchemaIntrospection::<$type_name$>::layout(ref layout);
                    array::ArrayTrait::span(@layout)
                }
            }

            impl $type_name$SchemaIntrospection of \
             dojo::database::schema::SchemaIntrospection<$type_name$> {
                #[inline(always)]
                fn size() -> usize {
                    $size$
                }

                #[inline(always)]
                fn layout(ref layout: Array<u8>) {
                    $layout$
                }

                #[inline(always)]
                fn ty() -> dojo::database::schema::Ty {
                    dojo::database::schema::Ty::Struct(dojo::database::schema::Struct {
                        name: '$type_name$',
                        attrs: array![].span(),
                        children: array![$member_types$].span()
                    })
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
                    dojo::database::schema::SchemaIntrospection::<$type_name$>::size()
                }

                #[external(v0)]
                fn layout(self: @ContractState) -> Span<u8> {
                    let mut layout = ArrayTrait::new();
                    dojo::database::schema::SchemaIntrospection::<$type_name$>::layout(ref layout);
                    array::ArrayTrait::span(@layout)
                }

                #[external(v0)]
                fn schema(self: @ContractState) -> dojo::database::schema::Ty {
                    dojo::database::schema::SchemaIntrospection::<$type_name$>::ty()
                }
            }
        ",
            UnorderedHashMap::from([
                ("contract_name".to_string(), RewriteNode::Text(name.to_case(Case::Snake))),
                (
                    "type_name".to_string(),
                    RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node()),
                ),
                ("serialized_keys".to_string(), RewriteNode::new_modified(serialized_keys)),
                ("serialized_values".to_string(), RewriteNode::new_modified(serialized_values)),
                ("layout".to_string(), RewriteNode::new_modified(layout)),
                ("member_types".to_string(), RewriteNode::Text(member_types.join(","))),
                (
                    "size".to_string(),
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
                                    "dojo::database::schema::SchemaIntrospection::<{}>::size()",
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

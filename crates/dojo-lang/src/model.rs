use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_syntax::node::ast::ItemStruct;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use convert_case::{Case, Casing};
use dojo_world::manifest::Member;
use dojo_world::migration::strategy::poseidon_hash_str;

use crate::introspect::handle_introspect_struct;
use crate::plugin::{DojoAuxData, Model};

/// A handler for Dojo code that modifies a model struct.
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the model struct.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_model_struct(
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
            message: "Model must define at least one #[key] attribute".into(),
            stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
        });
    }

    if keys.len() == members.len() {
        diagnostics.push(PluginDiagnostic {
            message: "Model must define at least one member that is not a key".into(),
            stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
        });
    }

    let serialize_member = |m: &Member, include_key: bool| {
        if m.key && !include_key {
            return None;
        }

        if m.ty == "felt252" {
            return Some(RewriteNode::Text(format!(
                "core::array::ArrayTrait::append(ref serialized, *self.{});",
                m.name
            )));
        }

        Some(RewriteNode::Text(format!(
            "core::serde::Serde::serialize(self.{}, ref serialized);",
            m.name
        )))
    };

    let serialized_keys: Vec<_> =
        keys.iter().filter_map(|m| serialize_member(m, true)).collect::<_>();

    let serialized_values: Vec<_> =
        members.iter().filter_map(|m| serialize_member(m, false)).collect::<_>();

    let name = struct_ast.name(db).text(db);
    let name_hash = format!("0x{:x}", poseidon_hash_str(name.as_str()));
    aux_data.models.push(Model { name: name.to_string(), members: members.to_vec() });

    (
        RewriteNode::interpolate_patched(
            "
            impl $name$Model of dojo::model::Model<$name$> {
                #[inline(always)]
                fn name_hash(self: @$name$) -> felt252 {
                    '$name$'
                }

                #[inline(always)]
                fn keys(self: @$name$) -> Span<felt252> {
                    let mut serialized = core::array::ArrayTrait::new();
                    $serialized_keys$
                    core::array::ArrayTrait::span(@serialized)
                }

                #[inline(always)]
                fn values(self: @$name$) -> Span<felt252> {
                    let mut serialized = core::array::ArrayTrait::new();
                    $serialized_values$
                    core::array::ArrayTrait::span(@serialized)
                }

                #[inline(always)]
                fn layout(self: @$name$) -> Span<u8> {
                    let mut layout = core::array::ArrayTrait::new();
                    dojo::database::introspect::Introspect::<$name$>::layout(ref layout);
                    core::array::ArrayTrait::span(@layout)
                }

                #[inline(always)]
                fn packed_size(self: @$name$) -> usize {
                    let mut layout = self.layout();
                    dojo::packing::calculate_packed_size(ref layout)
                }
            }

            $schema_introspection$

            #[starknet::interface]
            trait I$name$<T> {
                fn name(self: @T) -> felt252;
            }

            #[starknet::contract]
            mod $contract_name$ {
                use super::$name$;

                #[storage]
                struct Storage {}

                #[external(v0)]
                fn name_hash(self: @ContractState) -> felt252 {
                    $name_hash$
                }

                #[external(v0)]
                fn unpacked_size(self: @ContractState) -> usize {
                    dojo::database::introspect::Introspect::<$name$>::size()
                }

                #[external(v0)]
                fn packed_size(self: @ContractState) -> usize {
                    let mut layout = core::array::ArrayTrait::new();
                    dojo::database::introspect::Introspect::<$name$>::layout(ref layout);
                    let mut layout_span = layout.span();
                    dojo::packing::calculate_packed_size(ref layout_span)
                }

                #[external(v0)]
                fn layout(self: @ContractState) -> Span<u8> {
                    let mut layout = core::array::ArrayTrait::new();
                    dojo::database::introspect::Introspect::<$name$>::layout(ref layout);
                    core::array::ArrayTrait::span(@layout)
                }

                #[external(v0)]
                fn schema(self: @ContractState) -> dojo::database::introspect::Ty {
                    dojo::database::introspect::Introspect::<$name$>::ty()
                }
            }
        ",
            &UnorderedHashMap::from([
                ("contract_name".to_string(), RewriteNode::Text(name.to_case(Case::Snake))),
                ("name".to_string(), RewriteNode::Text(name.to_string())),
                ("name_hash".to_string(), RewriteNode::Text(name_hash.to_string())),
                ("schema_introspection".to_string(), handle_introspect_struct(db, struct_ast)),
                ("serialized_keys".to_string(), RewriteNode::new_modified(serialized_keys)),
                ("serialized_values".to_string(), RewriteNode::new_modified(serialized_values)),
            ]),
        ),
        diagnostics,
    )
}

use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::ItemStruct;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use convert_case::{Case, Casing};
use dojo_world::manifest::Member;

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
            severity: Severity::Error,
        });
    }

    if keys.len() == members.len() {
        diagnostics.push(PluginDiagnostic {
            message: "Model must define at least one member that is not a key".into(),
            stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
            severity: Severity::Error,
        });
    }

    for k in &keys {
        if k.ty == "u256" {
            diagnostics.push(PluginDiagnostic {
                message: "Key is only supported for core types that are 1 felt long once \
                          serialized. `u256` is a struct of 2 u128, hence not supported."
                    .into(),
                stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
                severity: Severity::Error,
            });
        }
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
    aux_data.models.push(Model { name: name.to_string(), members: members.to_vec() });

    (
        RewriteNode::interpolate_patched(
            "
            impl $type_name$Model of dojo::model::Model<$type_name$> {
                #[inline(always)]
                fn name(self: @$type_name$) -> felt252 {
                    '$type_name$'
                }

                #[inline(always)]
                fn keys(self: @$type_name$) -> Span<felt252> {
                    let mut serialized = core::array::ArrayTrait::new();
                    $serialized_keys$
                    core::array::ArrayTrait::span(@serialized)
                }

                #[inline(always)]
                fn values(self: @$type_name$) -> Span<felt252> {
                    let mut serialized = core::array::ArrayTrait::new();
                    $serialized_values$
                    core::array::ArrayTrait::span(@serialized)
                }

                #[inline(always)]
                fn layout(self: @$type_name$) -> Span<u8> {
                    let mut layout = core::array::ArrayTrait::new();
                    dojo::database::introspect::Introspect::<$type_name$>::layout(ref layout);
                    core::array::ArrayTrait::span(@layout)
                }

                #[inline(always)]
                fn packed_size(self: @$type_name$) -> usize {
                    let mut layout = self.layout();
                    dojo::packing::calculate_packed_size(ref layout)
                }
            }

            $schema_introspection$

            #[starknet::interface]
            trait I$contract_name$<T> {
                fn ensure_abi(self: @T, model: $type_name$);
            }

            #[starknet::contract]
            mod $contract_name$ {
                use super::$type_name$;
                use super::I$contract_name$;

                #[storage]
                struct Storage {}

                #[abi(embed_v0)]
                impl DojoModelImpl of dojo::model::IDojoModel<ContractState>{
                    fn name(self: @ContractState) -> felt252 {
                        '$type_name$'
                    }

                    fn unpacked_size(self: @ContractState) -> usize {
                        dojo::database::introspect::Introspect::<$type_name$>::size()
                    }

                    fn packed_size(self: @ContractState) -> usize {
                        let mut layout = core::array::ArrayTrait::new();
                        dojo::database::introspect::Introspect::<$type_name$>::layout(ref layout);
                        let mut layout_span = layout.span();
                        dojo::packing::calculate_packed_size(ref layout_span)
                    }

                    fn layout(self: @ContractState) -> Span<u8> {
                        let mut layout = core::array::ArrayTrait::new();
                        dojo::database::introspect::Introspect::<$type_name$>::layout(ref layout);
                        core::array::ArrayTrait::span(@layout)
                    }

                    fn schema(self: @ContractState) -> dojo::database::introspect::Ty {
                        dojo::database::introspect::Introspect::<$type_name$>::ty()
                    }
                }

                #[abi(embed_v0)]
                impl $contract_name$Impl of I$contract_name$<ContractState>{
                    fn ensure_abi(self: @ContractState, model: $type_name$) {
                    }
                }
            }
        ",
            &UnorderedHashMap::from([
                ("contract_name".to_string(), RewriteNode::Text(name.to_case(Case::Snake))),
                (
                    "type_name".to_string(),
                    RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node()),
                ),
                (
                    "schema_introspection".to_string(),
                    handle_introspect_struct(db, &mut diagnostics, struct_ast),
                ),
                ("serialized_keys".to_string(), RewriteNode::new_modified(serialized_keys)),
                ("serialized_values".to_string(), RewriteNode::new_modified(serialized_values)),
            ]),
        ),
        diagnostics,
    )
}

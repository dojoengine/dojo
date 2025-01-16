use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::Member as MemberAst;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedStablePtr, TypedSyntaxNode};
use dojo_types::naming::compute_bytearray_hash;
use starknet_crypto::{poseidon_hash_many, Felt};

use crate::aux_data::Member;

/// Compute a unique hash based on the element name and types and names of members.
/// This hash is used in element contracts to ensure uniqueness.
pub fn compute_unique_hash(
    db: &dyn SyntaxGroup,
    element_name: &str,
    is_packed: bool,
    members: &[MemberAst],
) -> Felt {
    let mut hashes =
        vec![if is_packed { Felt::ONE } else { Felt::ZERO }, compute_bytearray_hash(element_name)];
    hashes.extend(
        members
            .iter()
            .map(|m| {
                poseidon_hash_many(&[
                    compute_bytearray_hash(&m.name(db).text(db).to_string()),
                    compute_bytearray_hash(
                        m.type_clause(db).ty(db).as_syntax_node().get_text(db).trim(),
                    ),
                ])
            })
            .collect::<Vec<_>>(),
    );
    poseidon_hash_many(&hashes)
}

pub fn parse_members(
    db: &dyn SyntaxGroup,
    members: &[MemberAst],
    diagnostics: &mut Vec<PluginDiagnostic>,
) -> Vec<Member> {
    let mut parsing_keys = true;

    members
        .iter()
        .filter_map(|member_ast| {
            let is_key = member_ast.has_attr(db, "key");

            let member = Member {
                name: member_ast.name(db).text(db).to_string(),
                ty: member_ast
                    .type_clause(db)
                    .ty(db)
                    .as_syntax_node()
                    .get_text(db)
                    .trim()
                    .to_string(),
                key: is_key,
            };

            // Make sure all keys are before values in the model.
            if is_key && !parsing_keys {
                diagnostics.push(PluginDiagnostic {
                    message: "Key members must be defined before non-key members.".into(),
                    stable_ptr: member_ast.name(db).stable_ptr().untyped(),
                    severity: Severity::Error,
                });
                // Don't return here, since we don't want to stop processing the members after the first error to avoid
                // diagnostics just because the field is missing.
            }

            parsing_keys &= is_key;

            Some(member)
        })
        .collect::<Vec<_>>()
}

pub fn serialize_keys_and_values(
    members: &[Member],
    serialized_keys: &mut Vec<RewriteNode>,
    serialized_values: &mut Vec<RewriteNode>,
) {
    members.iter().for_each(|member| {
        if member.key {
            serialized_keys.push(serialize_member_ty(member, true));
        } else {
            serialized_values.push(serialize_member_ty(member, true));
        }
    });
}

pub fn deserialize_keys_and_values(
    members: &[Member],
    keys_input_name: &str,
    deserialized_keys: &mut Vec<RewriteNode>,
    values_input_name: &str,
    deserialized_values: &mut Vec<RewriteNode>,
) {
    members.iter().for_each(|member| {
        if member.key {
            deserialized_keys.push(deserialize_member_ty(member, keys_input_name));
        } else {
            deserialized_values.push(deserialize_member_ty(member, values_input_name));
        }
    });
}

/// Creates a [`RewriteNode`] for the member type serialization.
///
/// # Arguments
///
/// * member: The member to serialize.
pub fn serialize_member_ty(member: &Member, with_self: bool) -> RewriteNode {
    RewriteNode::Text(format!(
        "core::serde::Serde::serialize({}{}, ref serialized);\n",
        if with_self { "self." } else { "@" },
        member.name
    ))
}

pub fn deserialize_member_ty(member: &Member, input_name: &str) -> RewriteNode {
    RewriteNode::Text(format!(
        "let {} = core::serde::Serde::<{}>::deserialize(ref {input_name})?;\n",
        member.name, member.ty
    ))
}

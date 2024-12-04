use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_macro::{Diagnostic, ProcMacroResult, Severity, TokenStream};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::ast::{self, AttributeList, Member as MemberAst};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::kind::SyntaxKind::ItemStruct;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use dojo_types::naming;
use dojo_types::naming::compute_bytearray_hash;
use serde::{Deserialize, Serialize};
use starknet_crypto::{poseidon_hash_many, Felt};

use super::constants::DOJO_ATTR_NAMES;
use crate::diagnostic_ext::DiagnosticsExt;

/// Represents a member of a struct.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Member {
    // Name of the member.
    pub name: String,
    // Type of the member.
    pub ty: String,
    // Whether the member is a key.
    pub key: bool,
}

pub fn parse_members(
    db: &dyn SyntaxGroup,
    members: &[MemberAst],
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Member> {
    members
        .iter()
        .filter_map(|member_ast| {
            let member = Member {
                name: member_ast.name(db).text(db).to_string(),
                ty: member_ast
                    .type_clause(db)
                    .ty(db)
                    .as_syntax_node()
                    .get_text(db)
                    .trim()
                    .to_string(),
                key: member_ast.has_attr(db, "key"),
            };

            // validate key member
            if member.key && member.ty == "u256" {
                diagnostics.push(Diagnostic {
                    message: "Key is only supported for core types that are 1 felt long once \
                              serialized. `u256` is a struct of 2 u128, hence not supported."
                        .into(),
                    severity: Severity::Error,
                });
                None
            } else {
                Some(member)
            }
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

/// Validates the namings of the attributes.
///
/// # Arguments
///
/// * namings: A list of tuples containing the id and value of the attribute.
///
/// # Returns
///
/// A vector of diagnostics.
pub fn validate_namings_diagnostics(namings: &[(&str, &str)]) -> Vec<Diagnostic> {
    let mut diagnostics = vec![];

    for (id, value) in namings {
        if !naming::is_name_valid(value) {
            diagnostics.push_error(format!(
                "The {id} '{value}' can only contain characters (a-z/A-Z), digits (0-9) and \
                 underscore (_)."
            ));
        }
    }

    diagnostics
}

/// Removes the derives from the original struct.
pub fn remove_derives(db: &dyn SyntaxGroup, struct_ast: &ast::ItemStruct) -> RewriteNode {
    let mut out_lines = vec![];

    let struct_str = struct_ast.as_syntax_node().get_text_without_trivia(db).to_string();

    for l in struct_str.lines() {
        if !l.starts_with("#[derive") {
            out_lines.push(l);
        }
    }

    RewriteNode::Text(out_lines.join("\n"))
}

/// Validates the attributes of a Dojo attribute.
///
/// Parameters:
/// * db: The semantic database.
/// * module_ast: The AST of the contract module.
///
/// Returns:
/// * A vector of diagnostics.
pub fn validate_attributes(
    db: &dyn SyntaxGroup,
    attribute_list: &AttributeList,
    ref_attribute: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = vec![];

    for attribute in DOJO_ATTR_NAMES {
        if attribute == ref_attribute {
            if attribute_list.query_attr(db, attribute).first().is_some() {
                diagnostics.push_error(format!(
                    "Only one {} attribute is allowed per module.",
                    ref_attribute
                ));
            }
        } else if attribute_list.query_attr(db, attribute).first().is_some() {
            diagnostics.push_error(format!(
                "A {} can't be used together with a {}.",
                ref_attribute, attribute
            ));
        }
    }

    diagnostics
}

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

pub fn handle_struct_attribute_macro(
    token_stream: TokenStream,
    from_struct: fn(&dyn SyntaxGroup, &ast::ItemStruct) -> ProcMacroResult,
) -> ProcMacroResult {
    let db = SimpleParserDatabase::default();
    let (root_node, _diagnostics) = db.parse_virtual_with_diagnostics(token_stream);

    for n in root_node.descendants(&db) {
        if n.kind(&db) == ItemStruct {
            let struct_ast = ast::ItemStruct::from_syntax_node(&db, n);
            return from_struct(&db, &struct_ast);
        }
    }

    ProcMacroResult::new(TokenStream::empty())
}

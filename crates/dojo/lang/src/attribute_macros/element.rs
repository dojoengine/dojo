use std::collections::HashMap;

use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::{
    ArgClause, ArgClauseNamed, ItemStruct, Member as MemberAst, OptionArgListParenthesized,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedStablePtr, TypedSyntaxNode};

use crate::aux_data::Member;

/// `StructParameterParser` provides a general `from_struct` function to parse
/// the parameters of a struct attribute like dojo::model or dojo::event.
///
/// Processing of specific parameters can then be implemented through the `process_named_parameters`
/// function.
pub trait StructParameterParser {
    fn load_from_struct(
        &mut self,
        db: &dyn SyntaxGroup,
        attribute_name: &String,
        struct_ast: ItemStruct,
        diagnostics: &mut Vec<PluginDiagnostic>,
    ) {
        let mut processed_args: HashMap<String, bool> = HashMap::new();

        if let OptionArgListParenthesized::ArgListParenthesized(arguments) =
            struct_ast.attributes(db).query_attr(db, attribute_name).first().unwrap().arguments(db)
        {
            arguments.arguments(db).elements(db).iter().for_each(|a| match a.arg_clause(db) {
                ArgClause::Named(x) => {
                    let arg_name = x.name(db).text(db).to_string();

                    if processed_args.contains_key(&arg_name) {
                        diagnostics.push(PluginDiagnostic {
                            message: format!(
                                "Too many '{}' attributes for {attribute_name}",
                                arg_name
                            ),
                            stable_ptr: struct_ast.stable_ptr().untyped(),
                            severity: Severity::Error,
                        });
                    } else {
                        processed_args.insert(arg_name.clone(), true);
                        self.process_named_parameters(db, attribute_name, x, diagnostics);
                    }
                }
                ArgClause::Unnamed(x) => {
                    diagnostics.push(PluginDiagnostic {
                        message: format!(
                            "Unexpected argument '{}' for {attribute_name}",
                            x.as_syntax_node().get_text(db)
                        ),
                        stable_ptr: x.stable_ptr().untyped(),
                        severity: Severity::Warning,
                    });
                }
                ArgClause::FieldInitShorthand(x) => {
                    diagnostics.push(PluginDiagnostic {
                        message: format!(
                            "Unexpected argument '{}' for {attribute_name}",
                            x.name(db).name(db).text(db).to_string()
                        ),
                        stable_ptr: x.stable_ptr().untyped(),
                        severity: Severity::Warning,
                    });
                }
            })
        }
    }

    fn process_named_parameters(
        &mut self,
        db: &dyn SyntaxGroup,
        attribute_name: &str,
        arg: ArgClauseNamed,
        diagnostics: &mut Vec<PluginDiagnostic>,
    );
}

pub fn parse_members(
    db: &dyn SyntaxGroup,
    members: &[MemberAst],
    diagnostics: &mut Vec<PluginDiagnostic>,
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
                diagnostics.push(PluginDiagnostic {
                    message: "Key is only supported for core types that are 1 felt long once \
                              serialized. `u256` is a struct of 2 u128, hence not supported."
                        .into(),
                    stable_ptr: member_ast.name(db).stable_ptr().untyped(),
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

use std::collections::HashMap;

use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::{
    ArgClause, Expr, ItemStruct, Member as MemberAst, OptionArgListParenthesized,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedStablePtr, TypedSyntaxNode};
use dojo_world::config::NamespaceConfig;
use dojo_world::contracts::naming;
use dojo_world::manifest::Member;

pub const DEFAULT_DATA_VERSION: u8 = 1;

pub const DATA_VERSION_NAME: &str = "version";
pub const DATA_NAMESPACE: &str = "namespace";
pub const DATA_NOMAPPING: &str = "nomapping";

#[derive(Debug)]
pub struct DataParameters {
    pub version: u8,
    pub namespace: Option<String>,
    pub nomapping: bool,
}

impl Default for DataParameters {
    fn default() -> DataParameters {
        DataParameters { version: DEFAULT_DATA_VERSION, namespace: Option::None, nomapping: false }
    }
}

/// Get the version from the `Expr` parameter.
fn get_version(
    db: &dyn SyntaxGroup,
    attribute_name: &String,
    arg_value: Expr,
    diagnostics: &mut Vec<PluginDiagnostic>,
) -> u8 {
    match arg_value {
        Expr::Literal(ref value) => {
            if let Ok(value) = value.text(db).parse::<u8>() {
                if value <= DEFAULT_DATA_VERSION {
                    value
                } else {
                    diagnostics.push(PluginDiagnostic {
                        message: format!("{attribute_name} version {} not supported", value),
                        stable_ptr: arg_value.stable_ptr().untyped(),
                        severity: Severity::Error,
                    });
                    DEFAULT_DATA_VERSION
                }
            } else {
                diagnostics.push(PluginDiagnostic {
                    message: format!(
                        "The argument '{}' of {attribute_name} must be an integer",
                        DATA_VERSION_NAME
                    ),
                    stable_ptr: arg_value.stable_ptr().untyped(),
                    severity: Severity::Error,
                });
                DEFAULT_DATA_VERSION
            }
        }
        _ => {
            diagnostics.push(PluginDiagnostic {
                message: format!(
                    "The argument '{}' of {attribute_name} must be an integer",
                    DATA_VERSION_NAME
                ),
                stable_ptr: arg_value.stable_ptr().untyped(),
                severity: Severity::Error,
            });
            DEFAULT_DATA_VERSION
        }
    }
}

/// Get the namespace from the `Expr` parameter.
fn get_namespace(
    db: &dyn SyntaxGroup,
    attribute_name: &String,
    arg_value: Expr,
    diagnostics: &mut Vec<PluginDiagnostic>,
) -> Option<String> {
    match arg_value {
        Expr::ShortString(ss) => Some(ss.string_value(db).unwrap()),
        Expr::String(s) => Some(s.string_value(db).unwrap()),
        _ => {
            diagnostics.push(PluginDiagnostic {
                message: format!(
                    "The argument '{}' of {attribute_name} must be a string",
                    DATA_NAMESPACE
                ),
                stable_ptr: arg_value.stable_ptr().untyped(),
                severity: Severity::Error,
            });
            Option::None
        }
    }
}

/// Get parameters of a dojo attribute.
///
/// Note: dojo attribute has already been checked so there is one and only one attribute.
///
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the Dojo struct.
/// * diagnostics: vector of compiler diagnostics.
///
/// Returns:
/// * A [`DataParameters`] object containing all the Dojo attribute parameters with their default
///   values if not set in the code.
pub fn get_parameters(
    db: &dyn SyntaxGroup,
    attribute_name: &String,
    struct_ast: ItemStruct,
    diagnostics: &mut Vec<PluginDiagnostic>,
) -> DataParameters {
    let mut parameters = DataParameters::default();
    let mut processed_args: HashMap<String, bool> = HashMap::new();

    if let OptionArgListParenthesized::ArgListParenthesized(arguments) =
        struct_ast.attributes(db).query_attr(db, attribute_name).first().unwrap().arguments(db)
    {
        arguments.arguments(db).elements(db).iter().for_each(|a| match a.arg_clause(db) {
            ArgClause::Named(x) => {
                let arg_name = x.name(db).text(db).to_string();
                let arg_value = x.value(db);

                if processed_args.contains_key(&arg_name) {
                    diagnostics.push(PluginDiagnostic {
                        message: format!("Too many '{}' attributes for {attribute_name}", arg_name),
                        stable_ptr: struct_ast.stable_ptr().untyped(),
                        severity: Severity::Error,
                    });
                } else {
                    processed_args.insert(arg_name.clone(), true);

                    match arg_name.as_str() {
                        DATA_VERSION_NAME => {
                            parameters.version =
                                get_version(db, attribute_name, arg_value, diagnostics);
                        }
                        DATA_NAMESPACE => {
                            parameters.namespace =
                                get_namespace(db, attribute_name, arg_value, diagnostics);
                        }
                        DATA_NOMAPPING => {
                            parameters.nomapping = true;
                        }
                        _ => {
                            diagnostics.push(PluginDiagnostic {
                                message: format!(
                                    "Unexpected argument '{}' for {attribute_name}",
                                    arg_name
                                ),
                                stable_ptr: x.stable_ptr().untyped(),
                                severity: Severity::Warning,
                            });
                        }
                    }
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

    parameters
}

pub fn compute_namespace(
    element_name: &str,
    parameters: &DataParameters,
    namespace_config: &NamespaceConfig,
) -> String {
    let unmapped_namespace =
        parameters.namespace.clone().unwrap_or(namespace_config.default.clone());

    if parameters.nomapping {
        unmapped_namespace
    } else {
        // Maps namespace from the tag to ensure higher precision on matching namespace mappings.
        namespace_config.get_mapping(&naming::get_tag(&unmapped_namespace, element_name))
    }
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
    keys_dest_name: &str,
    serialized_keys: &mut Vec<RewriteNode>,
    values_dest_name: &str,
    serialized_values: &mut Vec<RewriteNode>,
) {
    members.iter().for_each(|member| {
        if member.key {
            serialized_keys.push(serialize_member_ty(member, true, keys_dest_name));
        } else {
            serialized_values.push(serialize_member_ty(member, true, values_dest_name));
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
pub fn serialize_member_ty(member: &Member, with_self: bool, dest_name: &str) -> RewriteNode {
    RewriteNode::Text(format!(
        "core::serde::Serde::serialize({}{}, ref {dest_name});\n",
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

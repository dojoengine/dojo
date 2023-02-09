use std::collections::HashMap;

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use starknet::core::crypto::pedersen_hash;
use starknet::core::types::FieldElement;
use starknet::core::utils::get_contract_address;

pub struct Query {
    pub rewrite_nodes: Vec<RewriteNode>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

impl Query {
    pub fn from_expr(db: &dyn SyntaxGroup, expr: ast::Expr) -> Self {
        let diagnostics = vec![];
        let rewrite_nodes: Vec<RewriteNode> = vec![];
        let mut query = Query { diagnostics, rewrite_nodes };

        match expr {
            ast::Expr::Path(path) => match &path.elements(db)[0] {
                ast::PathSegment::WithGenericArgs(segment) => {
                    let generic = segment.generic_args(db);
                    let parameters = generic.generic_args(db).elements(db);
                    for parameter in parameters {
                        query.handle_parameter(db, parameter);
                    }
                }
                _ => {
                    query.diagnostics.push(PluginDiagnostic {
                        message: "Invalid query type".to_string(),
                        stable_ptr: path.stable_ptr().untyped(),
                    });
                }
            },
            _ => {
                query.diagnostics.push(PluginDiagnostic {
                    message: "Invalid query type".to_string(),
                    stable_ptr: expr.stable_ptr().untyped(),
                });
            }
        }
        query
    }

    fn handle_parameter(&mut self, db: &dyn SyntaxGroup, parameter: ast::Expr) {
        match parameter {
            ast::Expr::Tuple(tuple) => {
                for element in tuple.expressions(db).elements(db) {
                    self.handle_parameter(db, element);
                }
            }

            ast::Expr::Path(path) => {
                let var_prefix = match path.elements(db).last() {
                    Some(segment) => segment.as_syntax_node().get_text(db).to_ascii_lowercase(),
                    None => {
                        return self.diagnostics.push(PluginDiagnostic {
                            message: "Resolving query name.".to_string(),
                            stable_ptr: path.stable_ptr().untyped(),
                        });
                    }
                };

                let class_hash = "0x00000000000000000000000000000000";
                // TODO(https://github.com/dojoengine/dojo/issues/38): Move to cairo_project.toml
                let world_address = "0x00000000000000000000000000000000";

                // Component name to felt
                let component_name_raw = path.as_syntax_node().get_text(db);
                let mut component_name_parts: Vec<&str> = component_name_raw.split("::").collect();
                let component_name = component_name_parts.pop().unwrap();

                let mut component_name_32_u8: [u8; 32] = [0; 32];
                component_name_32_u8[32 - component_name.len()..]
                    .copy_from_slice(component_name.as_bytes());

                // Component name pedersen salt
                let salt = pedersen_hash(
                    &FieldElement::ZERO,
                    &FieldElement::from_bytes_be(&component_name_32_u8).unwrap(),
                );
                let component_id = format!(
                    "{:#x}",
                    get_contract_address(
                        salt,
                        FieldElement::from_hex_be(class_hash).unwrap(),
                        &[],
                        FieldElement::from_hex_be(world_address).unwrap(),
                    )
                );

                self.rewrite_nodes.push(RewriteNode::interpolate_patched(
                    "let $var_prefix$_ids = super::IWorldDispatcher::lookup(world, \
                     $component_address$);",
                    HashMap::from([
                        ("var_prefix".to_string(), RewriteNode::Text(var_prefix)),
                        ("component_address".to_string(), RewriteNode::Text(component_id)),
                    ]),
                ))
            }
            _ => {
                self.diagnostics.push(PluginDiagnostic {
                    message: "Unsupported query type. Must be tuple or single struct.".to_string(),
                    stable_ptr: parameter.stable_ptr().untyped(),
                });
            }
        }
    }
}

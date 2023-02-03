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
        query.handle_expression(db, expr);
        query
    }

    fn handle_expression(&mut self, db: &dyn SyntaxGroup, expression: ast::Expr) {
        match expression {
            ast::Expr::Tuple(tuple) => {
                for element in tuple.expressions(db).elements(db) {
                    self.handle_expression(db, element);
                }
            }

            ast::Expr::Path(path) => {
                let binding = path.elements(db);
                let last = binding.last().unwrap();
                match last {
                    ast::PathSegment::WithGenericArgs(segment) => {
                        let generic = segment.generic_args(db);
                        let parameters = generic.generic_args(db).elements(db);
                        for parameter in parameters {
                            self.handle_expression(db, parameter);
                        }
                    }
                    ast::PathSegment::Simple(segment) => {
                        let var_prefix = segment.as_syntax_node().get_text(db).to_ascii_lowercase();

                        let class_hash = "0x00000000000000000000000000000000";
                        let world_address = "0x00000000000000000000000000000000";

                        // Component name to felt
                        let component_name = path.as_syntax_node().get_text(db);
                        let mut component_name_32_u8: [u8; 32] = [0; 32];
                        component_name_32_u8[32 - component_name.len()..]
                            .copy_from_slice(&component_name.as_bytes());

                        // Component name pedersen salt
                        let salt = pedersen_hash(
                            &FieldElement::ZERO,
                            &FieldElement::from_bytes_be(&component_name_32_u8).unwrap(),
                        );
                        let component_id = get_contract_address(
                            salt,
                            FieldElement::from_hex_be(class_hash).unwrap(),
                            &vec![],
                            FieldElement::from_hex_be(world_address).unwrap(),
                        )
                        .to_string();

                        self.rewrite_nodes.push(RewriteNode::interpolate_patched(
                            "let $var_prefix$_ids = IWorld.lookup(world, $component_address$);",
                            HashMap::from([
                                ("var_prefix".to_string(), RewriteNode::Text(var_prefix)),
                                ("component_address".to_string(), RewriteNode::Text(component_id)),
                            ]),
                        ))
                    }
                }
            }
            _ => {
                self.diagnostics.push(PluginDiagnostic {
                    message: "Unsupported query type. Must be tuple or single struct.".to_string(),
                    stable_ptr: expression.stable_ptr().untyped(),
                });
            }
        }
    }
}

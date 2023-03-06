use std::collections::HashMap;

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use dojo_project::WorldConfig;

use crate::plugin::get_contract_address;

pub struct Query {
    pub world_config: WorldConfig,
    pub rewrite_nodes: Vec<RewriteNode>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

impl Query {
    pub fn from_expr(db: &dyn SyntaxGroup, world_config: WorldConfig, expr: ast::Expr) -> Self {
        let diagnostics = vec![];
        let rewrite_nodes: Vec<RewriteNode> = vec![];
        let mut query = Query { world_config, diagnostics, rewrite_nodes };
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

                        // Component name to felt
                        let component_name_raw = path.as_syntax_node().get_text(db);
                        let mut component_name_parts: Vec<&str> =
                            component_name_raw.split("::").collect();
                        let component_name = component_name_parts.pop().unwrap();

                        let component_id = format!(
                            "{:#x}",
                            get_contract_address(
                                component_name,
                                self.world_config.initializer_class_hash.unwrap_or_default(),
                                self.world_config.address.unwrap_or_default(),
                            )
                        );

                        self.rewrite_nodes.push(RewriteNode::interpolate_patched(
                            "let $var_prefix$_ids = IWorldDispatcher { contract_address: \
                             world_address }.entities(starknet::contract_address_const::<$component_address$>());\n",
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

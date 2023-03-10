use std::collections::HashMap;

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use dojo_project::WorldConfig;
use smol_str::SmolStr;

use crate::component::compute_component_id;

pub struct Query {
    pub world_config: WorldConfig,
    pub rewrite_nodes: Vec<RewriteNode>,
    pub diagnostics: Vec<PluginDiagnostic>,
    imports: Vec<SmolStr>,
}

impl Query {
    pub fn from_expr(db: &dyn SyntaxGroup, world_config: WorldConfig, expr: ast::Expr) -> Self {
        let diagnostics = vec![];
        let rewrite_nodes: Vec<RewriteNode> = vec![];
        let mut query = Query { world_config, diagnostics, rewrite_nodes, imports: vec![] };
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
                        let component_id = compute_component_id(db, path, self.world_config);
                        self.imports.push(segment.ident(db).text(db));
                        self.rewrite_nodes.push(RewriteNode::interpolate_patched(
                            "let $var_prefix$_ids = IWorldDispatcher { contract_address: \
                             world_address \
                             }.entities(starknet::contract_address_const::<$component_address$>());\
                             \n",
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

    pub fn imports(&self) -> Vec<RewriteNode> {
        self.imports
            .iter()
            .map(|import| {
                RewriteNode::interpolate_patched(
                    "use super::$import$;",
                    HashMap::from([("import".to_string(), RewriteNode::Text(import.to_string()))]),
                )
            })
            .collect()
    }
}

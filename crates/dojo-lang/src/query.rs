use std::collections::{HashMap, HashSet};

use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use dojo_project::WorldConfig;
use smol_str::SmolStr;

use crate::plugin::get_contract_address;

pub enum Constraint {
    Has,
}

pub struct Fragment {
    pub component: SmolStr,
    pub constraint: Constraint,
}

pub struct Query {
    pub dependencies: HashSet<SmolStr>,
    pub fragments: Vec<Fragment>,
}

impl Query {
    pub fn from_expr(db: &dyn SyntaxGroup, query_ast: ast::StatementLet) -> Self {
        let mut query = Query { dependencies: HashSet::new(), fragments: vec![] };

        if let ast::Expr::FunctionCall(expr_fn) = query_ast.rhs(db) {
            if let Some(ast::PathSegment::WithGenericArgs(segment_genric)) =
                expr_fn.path(db).elements(db).first()
            {
                if segment_genric.ident(db).text(db) == "QueryTrait" {
                    for arg in segment_genric.generic_args(db).generic_args(db).elements(db) {
                        if let ast::GenericArg::Expr(expr) = arg {
                            query.handle_expression(db, expr.value(db));
                        }
                    }
                }
            }
        }

        query
    }

    pub fn nodes(self, world_config: WorldConfig) -> Vec<RewriteNode> {
        self.fragments
            .iter()
            .map(|fragment| {
                let component_address = format!(
                    "{:#x}",
                    get_contract_address(
                        fragment.component.as_str(),
                        world_config.initializer_class_hash.unwrap_or_default(),
                        world_config.address.unwrap_or_default(),
                    )
                );
                RewriteNode::interpolate_patched(
                    "let $var_prefix$_ids = IWorldDispatcher { contract_address: world_address \
                     }.entities(starknet::contract_address_const::<$component_address$>());\n",
                    HashMap::from([
                        (
                            "var_prefix".to_string(),
                            RewriteNode::Text(fragment.component.to_string().to_ascii_lowercase()),
                        ),
                        ("component_address".to_string(), RewriteNode::Text(component_address)),
                    ]),
                )
            })
            .collect::<Vec<_>>()
    }

    fn handle_expression(&mut self, db: &dyn SyntaxGroup, expression: ast::Expr) {
        match expression {
            ast::Expr::Tuple(tuple) => {
                for element in tuple.expressions(db).elements(db) {
                    self.handle_expression(db, element);
                }
            }
            ast::Expr::Parenthesized(parenthesized) => {
                self.handle_expression(db, parenthesized.expr(db));
            }
            ast::Expr::Path(path) => match path.elements(db).last().unwrap() {
                ast::PathSegment::WithGenericArgs(segment) => {
                    let generic = segment.generic_args(db);

                    for param in generic.generic_args(db).elements(db) {
                        if let ast::GenericArg::Expr(expr) = param {
                            self.handle_expression(db, expr.value(db));
                        }
                    }

                    self.dependencies.insert(segment.ident(db).text(db));
                }
                ast::PathSegment::Simple(segment) => {
                    self.dependencies.insert(segment.ident(db).text(db));
                    self.fragments.push(Fragment {
                        component: segment.ident(db).text(db),
                        constraint: Constraint::Has,
                    });
                }
            },
            _ => {
                unimplemented!(
                    "Unsupported expression type: {}",
                    expression.as_syntax_node().get_text(db)
                );
            }
        }
    }
}

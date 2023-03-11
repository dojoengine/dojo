use std::collections::HashSet;

use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use smol_str::SmolStr;

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
    pub fn from_expr(db: &dyn SyntaxGroup, expr: ast::Expr) -> Self {
        let mut query = Query { dependencies: HashSet::new(), fragments: vec![] };
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

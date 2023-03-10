use std::collections::HashSet;

use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use smol_str::SmolStr;

pub struct Query {
    pub dependencies: HashSet<SmolStr>,
    pub components: HashSet<SmolStr>,
}

impl Query {
    pub fn from_expr(db: &dyn SyntaxGroup, expr: ast::Expr) -> Self {
        let mut query = Query { dependencies: HashSet::new(), components: HashSet::new() };
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

                        let typ = segment.ident(db).text(db);
                        if typ == "Option" {
                            return;
                        }

                        self.dependencies.insert(segment.ident(db).text(db));
                    }
                    ast::PathSegment::Simple(segment) => {
                        // let var_prefix = segment.as_syntax_node().get_text(db).to_ascii_lowercase();
                        self.dependencies.insert(segment.ident(db).text(db));
                        self.components.insert(segment.ident(db).text(db));
                    }
                }
            }
            _ => {
                unimplemented!(
                    "Unsupported expression type: {}",
                    expression.as_syntax_node().get_text(db)
                );
            }
        }
    }
}

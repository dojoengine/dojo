use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use smol_str::SmolStr;

pub mod get;
pub mod set;

const CAIRO_ERR_MSG_LEN: usize = 31;

pub fn extract_components(db: &dyn SyntaxGroup, expression: &ast::Expr) -> Vec<SmolStr> {
    let mut components = vec![];
    match expression {
        ast::Expr::Tuple(tuple) => {
            for element in tuple.expressions(db).elements(db) {
                components.extend(extract_components(db, &element));
            }
        }
        ast::Expr::Parenthesized(parenthesized) => {
            components.extend(extract_components(db, &parenthesized.expr(db)));
        }
        ast::Expr::Path(path) => match path.elements(db).last().unwrap() {
            ast::PathSegment::WithGenericArgs(segment) => {
                let generic = segment.generic_args(db);

                for param in generic.generic_args(db).elements(db) {
                    if let ast::GenericArg::Expr(expr) = param {
                        components.extend(extract_components(db, &expr.value(db)));
                    }
                }
            }
            ast::PathSegment::Simple(segment) => {
                components.push(segment.ident(db).text(db));
            }
        },
        _ => {
            unimplemented!(
                "Unsupported expression type: {}",
                expression.as_syntax_node().get_text(db)
            );
        }
    }

    components
}

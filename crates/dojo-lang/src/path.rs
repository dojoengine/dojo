use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};

pub fn expand_path(
    db: &dyn SyntaxGroup,
    arg: ast::Arg,
    length: usize,
) -> Result<RewriteNode, PluginDiagnostic> {
    let mut entity_path: Vec<String> = vec!["0".to_string(); length];
    if let ast::ArgClause::Unnamed(path) = arg.arg_clause(db) {
        match path.value(db) {
            ast::Expr::Parenthesized(bundle) => {
                entity_path[length - 1] = bundle.expr(db).as_syntax_node().get_text(db);
                return Ok(RewriteNode::Text(format!("({})", entity_path.join(", "))));
            }
            ast::Expr::Tuple(tuple) => {
                let mut elements = tuple.expressions(db).elements(db);

                if elements.len() > 4 {
                    return Err(PluginDiagnostic {
                        message: "Entity path too long".to_string(),
                        stable_ptr: arg.as_syntax_node().stable_ptr(),
                    });
                }

                elements.reverse();
                for (count, expr) in elements.into_iter().enumerate() {
                    let index = length - 1 - count;
                    entity_path[index] = expr.as_syntax_node().get_text(db);
                }
                return Ok(RewriteNode::Text(format!("({})", entity_path.join(", "))));
            }
            ast::Expr::Literal(literal) => {
                entity_path[length - 1] = literal.text(db).to_string();
                return Ok(RewriteNode::Text(format!("({})", entity_path.join(", "))));
            }
            ast::Expr::Path(path) => {
                return Ok(RewriteNode::new_trimmed(path.as_syntax_node()));
            }
            _ => {
                return Err(PluginDiagnostic {
                    message: "Invalid entity path".to_string(),
                    stable_ptr: arg.as_syntax_node().stable_ptr(),
                });
            }
        }
    }

    Err(PluginDiagnostic {
        message: "Invalid entity path".to_string(),
        stable_ptr: arg.as_syntax_node().stable_ptr(),
    })
}

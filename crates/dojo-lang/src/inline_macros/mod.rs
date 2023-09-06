use cairo_lang_defs::plugin::{InlinePluginResult, PluginDiagnostic};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use smol_str::SmolStr;

pub mod emit;
pub mod get;
pub mod set;

const CAIRO_ERR_MSG_LEN: usize = 31;

pub fn extract_components(
    db: &dyn SyntaxGroup,
    expression: &ast::Expr,
) -> Result<Vec<SmolStr>, PluginDiagnostic> {
    let mut components = vec![];
    match expression {
        ast::Expr::Tuple(tuple) => {
            for element in tuple.expressions(db).elements(db) {
                match extract_components(db, &element) {
                    Ok(mut element_components) => components.append(&mut element_components),
                    Err(diagnostic) => return Err(diagnostic),
                }
            }
        }
        ast::Expr::Parenthesized(parenthesized) => {
            match extract_components(db, &parenthesized.expr(db)) {
                Ok(mut parenthesized_components) => {
                    components.append(&mut parenthesized_components)
                }
                Err(diagnostic) => return Err(diagnostic),
            }
        }
        ast::Expr::Path(path) => match path.elements(db).last().unwrap() {
            ast::PathSegment::WithGenericArgs(segment) => {
                let generic = segment.generic_args(db);

                for param in generic.generic_args(db).elements(db) {
                    let ast::GenericArg::Unnamed(unnamed) = param else {
                        return Err(PluginDiagnostic {
                            stable_ptr: param.stable_ptr().untyped(),
                            message: "Should be an unnamed argument".to_string(),
                        });
                    };

                    let ast::GenericArgValue::Expr(expr) = unnamed.value(db) else {
                        return Err(PluginDiagnostic {
                            stable_ptr: unnamed.stable_ptr().untyped(),
                            message: "Should be an expression".to_string(),
                        });
                    };

                    match extract_components(db, &expr.expr(db)) {
                        Ok(mut expr_components) => components.append(&mut expr_components),
                        Err(diagnostic) => return Err(diagnostic),
                    }
                }
            }
            ast::PathSegment::Simple(segment) => {
                components.push(segment.ident(db).text(db));
            }
        },
        _ => {
            return Err(PluginDiagnostic {
                stable_ptr: expression.stable_ptr().untyped(),
                message: format!(
                    "Unsupported expression type: {}",
                    expression.as_syntax_node().get_text(db)
                ),
            });
        }
    }

    Ok(components)
}
pub fn unsupported_arg_diagnostic(
    db: &dyn SyntaxGroup,
    macro_ast: &ast::ExprInlineMacro,
) -> InlinePluginResult {
    InlinePluginResult {
        code: None,
        diagnostics: vec![PluginDiagnostic {
            stable_ptr: macro_ast.stable_ptr().untyped(),
            message: format!(
                "Macro {} does not support this arg type",
                macro_ast.path(db).as_syntax_node().get_text(db)
            ),
        }],
    }
}

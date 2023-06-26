use cairo_lang_syntax::node::ast::{self, PathSegmentSimple};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::Terminal;
use smol_str::SmolStr;

pub fn macro_name(db: &dyn SyntaxGroup, macro_ast: ast::ExprInlineMacro) -> SmolStr {
    let elements = macro_ast.path(db).elements(db);
    let segment = elements.last().unwrap();
    match segment {
        ast::PathSegment::Simple(method) => method.ident(db).text(db),
        _ => panic!("Macro's name must be a simple identifier!"),
    }
}

pub fn ast_arg_to_expr(db: &dyn SyntaxGroup, arg: &ast::Arg) -> Option<ast::Expr> {
    match arg.arg_clause(db) {
        ast::ArgClause::Unnamed(clause) => Some(clause.value(db)),
        _ => None,
    }
}

fn ast_arg_to_path_segment_simple(
    db: &dyn SyntaxGroup,
    arg: &ast::Arg,
) -> Option<PathSegmentSimple> {
    if let Some(ast::Expr::Path(path)) = ast_arg_to_expr(db, arg) {
        if path.elements(db).len() != 1 {
            return None;
        }
        if let Some(ast::PathSegment::Simple(segment)) = path.elements(db).last() {
            return Some(segment.clone());
        }
    }
    None
}

pub fn context_arg_as_path_segment_simple_or_panic(
    db: &dyn SyntaxGroup,
    context: &ast::Arg,
) -> PathSegmentSimple {
    ast_arg_to_path_segment_simple(db, context).expect("Context must be a simple literal!")
}

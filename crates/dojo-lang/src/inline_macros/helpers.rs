use cairo_lang_syntax::node::ast::{self};
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

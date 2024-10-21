use cairo_lang_defs::ids::FunctionWithBodyId;
use cairo_lang_semantic as semantic;
use cairo_lang_syntax::node::{ast, SyntaxNode, TypedSyntaxNode};
use semantic::db::SemanticGroup;
use semantic::items::function_with_body::SemanticExprLookup;

/// Returns the semantic expression for the current node.
pub fn nearest_semantic_expr(
    db: &dyn SemanticGroup,
    mut node: SyntaxNode,
    function_id: FunctionWithBodyId,
) -> Option<cairo_lang_semantic::Expr> {
    loop {
        let syntax_db = db.upcast();
        if ast::Expr::is_variant(node.kind(syntax_db)) {
            let expr_node = ast::Expr::from_syntax_node(syntax_db, node.clone());
            if let Ok(expr_id) = db.lookup_expr_by_ptr(function_id, expr_node.stable_ptr()) {
                let semantic_expr = db.expr_semantic(function_id, expr_id);
                return Some(semantic_expr);
            }
        }
        node = node.parent()?;
    }
}

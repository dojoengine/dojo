use cairo_lang_syntax::node::ast::OptionTypeClause;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};

/// Gets the name, modifiers and type of a function parameter.
///
/// # Arguments
///
/// * `db` - The syntax group.
/// * `param` - The parameter.
///
/// # Returns
///
/// * A tuple containing the name, modifiers and type of the parameter.
pub fn get_parameter_info(db: &dyn SyntaxGroup, param: ast::Param) -> (String, String, String) {
    let name = param.name(db).text(db).trim().to_string();
    let modifiers = param
        .modifiers(db)
        .as_syntax_node()
        .get_text(db)
        .trim()
        .to_string();

    let param_type = if let OptionTypeClause::TypeClause(ty) = param.type_clause(db) {
        ty.ty(db).as_syntax_node().get_text(db).trim().to_string()
    } else {
        "()".to_string()
    };

    (name, modifiers, param_type)
}

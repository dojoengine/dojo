use cairo_lang_syntax::node::ast;
use cairo_lang_syntax::node::db::SyntaxGroup;

use super::utils as syntax_utils;

pub const SELF_PARAM_NAME: &str = "self";

/// Checks if the given function parameter is using `self` instead of `world` param.
/// Adds diagnostic if that case.
///
/// # Arguments
///
/// - `db` - The syntax group.
/// - `param_list` - The parameter list of the function.
/// - `fn_diagnostic_item` - The diagnostic item of the function.
/// - `diagnostics` - The diagnostics vector.
///
/// # Returns
///
/// - `true` if the function first parameter is `self`.
pub fn check_parameter(db: &dyn SyntaxGroup, param_list: &ast::ParamList) -> bool {
    if param_list.elements(db).is_empty() {
        return false;
    }

    let param_0 = param_list.elements(db)[0].clone();
    let (name, _, _) = syntax_utils::get_parameter_info(db, param_0.clone());

    name.eq(SELF_PARAM_NAME)
}

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, ids};

use crate::syntax::utils as syntax_utils;

const SELF_PARAM_NAME: &str = "self";

/// Checks if the given function is not using `self` param.
///
/// # Arguments
///
/// - `db` - The syntax group.
/// - `param_list` - The parameter list of the function.
/// - `fn_diagnostic_item` - The diagnostic item of the function.
/// - `diagnostics` - The diagnostics vector.
pub fn check_self_parameter(
    db: &dyn SyntaxGroup,
    param_list: &ast::ParamList,
    fn_diagnostic_item: ids::SyntaxStablePtrId,
    diagnostics: &mut Vec<PluginDiagnostic>,
) {
    if param_list.elements(db).is_empty() {
        return;
    }

    let param_0 = param_list.elements(db)[0].clone();
    let (name, modifier, _) = syntax_utils::get_parameter_info(db, param_0.clone());

    if name.eq(SELF_PARAM_NAME) {
        let (expected, actual) = if modifier.eq(&"ref".to_string()) {
            ("ref world: IWorldDispatcher", "ref self: ContractState")
        } else {
            ("world: @IWorldDispatcher", "self: @ContractState")
        };

        diagnostics.push(PluginDiagnostic {
            stable_ptr: fn_diagnostic_item,
            message: format!(
                "In a dojo contract or interface, you should use `{}` instead of `{}`.",
                expected, actual
            )
            .to_string(),
            severity: Severity::Error,
        });
    }
}

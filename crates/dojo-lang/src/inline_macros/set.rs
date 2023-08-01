use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};

use crate::inline_macro_plugin::{InlineMacro, InlineMacroExpanderData};

pub struct SetMacro;
impl InlineMacro for SetMacro {
    fn append_macro_code(
        &self,
        macro_expander_data: &mut InlineMacroExpanderData,
        db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
        macro_arguments: &cairo_lang_syntax::node::ast::ExprList,
    ) {
        let args = macro_arguments.elements(db);

        if args.len() != 2 {
            macro_expander_data.diagnostics.push(PluginDiagnostic {
                message: "Invalid arguments. Expected \"(world, (components,))\"".to_string(),
                stable_ptr: macro_arguments.as_syntax_node().stable_ptr(),
            });
            return;
        }

        let world = &args[0];
        let mut bundle = vec![];

        match &args[1] {
            ast::Expr::Parenthesized(parens) => {
                bundle.push(parens.expr(db).as_syntax_node().get_text(db))
            }
            ast::Expr::Tuple(list) => list.expressions(db).elements(db).iter().for_each(|expr| {
                bundle.push(expr.as_syntax_node().get_text(db));
            }),
            ast::Expr::StructCtorCall(ctor) => bundle.push(ctor.as_syntax_node().get_text(db)),
            _ => {
                macro_expander_data.diagnostics.push(PluginDiagnostic {
                    message: "Invalid arguments. Expected \"(world, query, (components,))\""
                        .to_string(),
                    stable_ptr: macro_arguments.as_syntax_node().stable_ptr(),
                });
                return;
            }
        }

        let mut expanded_code = "{".to_string();
        for entity in bundle {
            expanded_code.push_str(&format!(
                "\n            {}.set_entity({}.name(), {}.keys(), 0_u8, {}.values());",
                world.as_syntax_node().get_text(db),
                entity,
                entity,
                entity,
            ));
        }
        expanded_code.push('}');
        macro_expander_data.result_code.push_str(&expanded_code);
        macro_expander_data.code_changed = true;
    }

    fn is_bracket_type_allowed(
        &self,
        db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
        macro_ast: &cairo_lang_syntax::node::ast::ExprInlineMacro,
    ) -> bool {
        matches!(
            macro_ast.arguments(db),
            cairo_lang_syntax::node::ast::WrappedExprList::ParenthesizedExprList(_)
        )
    }
}

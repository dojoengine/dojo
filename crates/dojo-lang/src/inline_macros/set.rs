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
        println!("set macro");
        let args = macro_arguments.elements(db);

        if args.len() != 3 {
            macro_expander_data.diagnostics.push(PluginDiagnostic {
                message: "Invalid arguments. Expected \"(world, query, (components,))\""
                    .to_string(),
                stable_ptr: macro_arguments.as_syntax_node().stable_ptr(),
            });
            return;
        }

        let world = &args[0];
        let query = &args[1].clone();
        let mut bundle = vec![];

        match &args[2] {
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

        let mut expanded_code = format!(
            "{{
            let query = {};
            ",
            query.as_syntax_node().get_text(db)
        )
        .to_string();
        for entity in bundle {
            expanded_code.push_str(&format!(
                "\n            let mut __set_macro_calldata__ = ArrayTrait::new();
                let __set_macro__value__ = {};
                serde::Serde::serialize(@__set_macro__value__, ref __set_macro_calldata__);
                {}.set_entity(dojo::traits::Component::name(@__set_macro__value__), query, 0_u8, \
                 array::ArrayTrait::span(@__set_macro_calldata__));",
                entity,
                world.as_syntax_node().get_text(db),
            ));
        }
        expanded_code.push_str("}");
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

use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, PluginDiagnostic, PluginGeneratedFile,
};
use cairo_lang_semantic::inline_macros::unsupported_bracket_diagnostic;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};

use super::unsupported_arg_diagnostic;

#[derive(Debug)]
pub struct SetMacro;
impl SetMacro {
    pub const NAME: &'static str = "set";
}
impl InlineMacroExprPlugin for SetMacro {
    fn generate_code(
        &self,
        db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
        syntax: &ast::ExprInlineMacro,
    ) -> InlinePluginResult {
        let ast::WrappedArgList::ParenthesizedArgList(arg_list) = syntax.arguments(db) else {
            return unsupported_bracket_diagnostic(db, syntax);
        };
        let mut builder = PatchBuilder::new(db);
        builder.add_str("{");

        let args = arg_list.args(db).elements(db);

        if args.len() != 2 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: arg_list.args(db).stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \"(world, (components,))\"".to_string(),
                }],
            };
        }

        let world = &args[0];

        let ast::ArgClause::Unnamed(components) = args[1].arg_clause(db) else {
            return unsupported_arg_diagnostic(db, syntax);
        };

        let mut bundle = vec![];

        match components.value(db) {
            ast::Expr::Parenthesized(parens) => {
                bundle.push(parens.expr(db).as_syntax_node().get_text(db))
            }
            ast::Expr::Tuple(list) => list.expressions(db).elements(db).iter().for_each(|expr| {
                bundle.push(expr.as_syntax_node().get_text(db));
            }),
            ast::Expr::StructCtorCall(ctor) => bundle.push(ctor.as_syntax_node().get_text(db)),
            _ => {
                return InlinePluginResult {
                    code: None,
                    diagnostics: vec![PluginDiagnostic {
                        message: "Invalid arguments. Expected \"(world, (components,))\""
                            .to_string(),
                        stable_ptr: arg_list.args(db).stable_ptr().untyped(),
                    }],
                };
            }
        }

        if bundle.is_empty() {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    message: "Invalid arguments: No components provided.".to_string(),
                    stable_ptr: arg_list.args(db).stable_ptr().untyped(),
                }],
            };
        }

        for entity in bundle {
            builder.add_str(&format!(
                "\n            let __set_macro_value__ = {};
                {}.set_entity(dojo::component::Component::name(@__set_macro_value__), \
                 dojo::component::Component::keys(@__set_macro_value__), 0_u8, \
                 dojo::component::Component::pack(@__set_macro_value__));",
                entity,
                world.as_syntax_node().get_text(db),
            ));
        }
        builder.add_str("}");

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "set_inline_macro".into(),
                content: builder.code,
                diagnostics_mappings: builder.diagnostics_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

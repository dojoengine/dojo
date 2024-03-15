use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, NamedPlugin, PluginDiagnostic, PluginGeneratedFile,
};
use cairo_lang_diagnostics::Severity;
use cairo_lang_semantic::inline_macros::unsupported_bracket_diagnostic;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};

use super::unsupported_arg_diagnostic;

#[derive(Debug, Default)]
pub struct EmitMacro;

impl NamedPlugin for EmitMacro {
    const NAME: &'static str = "emit";
}

impl InlineMacroExprPlugin for EmitMacro {
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

        let args = arg_list.arguments(db).elements(db);

        if args.len() != 2 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: arg_list.arguments(db).stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \"emit!(world, models,)\"".to_string(),
                    severity: Severity::Error,
                }],
            };
        }

        let world = &args[0];

        let ast::ArgClause::Unnamed(models) = args[1].arg_clause(db) else {
            return unsupported_arg_diagnostic(db, syntax);
        };

        let mut bundle = vec![];

        match models.value(db) {
            ast::Expr::Parenthesized(parens) => {
                let syntax_node = parens.expr(db).as_syntax_node();
                bundle.push((syntax_node.get_text(db), syntax_node));
            }
            ast::Expr::Tuple(list) => {
                list.expressions(db).elements(db).into_iter().for_each(|expr| {
                    let syntax_node = expr.as_syntax_node();
                    bundle.push((syntax_node.get_text(db), syntax_node));
                })
            }
            ast::Expr::StructCtorCall(ctor) => {
                let syntax_node = ctor.as_syntax_node();
                bundle.push((syntax_node.get_text(db), syntax_node));
            }
            _ => {
                return InlinePluginResult {
                    code: None,
                    diagnostics: vec![PluginDiagnostic {
                        message: "Invalid arguments. Expected \"(world, (models,))\"".to_string(),
                        stable_ptr: arg_list.arguments(db).stable_ptr().untyped(),
                        severity: Severity::Error,
                    }],
                };
            }
        }

        if bundle.is_empty() {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    message: "Invalid arguments: No models provided.".to_string(),
                    stable_ptr: arg_list.arguments(db).stable_ptr().untyped(),
                    severity: Severity::Error,
                }],
            };
        }

        for (event, _) in bundle {
            builder.add_str("{");

            builder.add_str(
                "
                let mut keys = Default::<core::array::Array>::default();
                let mut data = Default::<core::array::Array>::default();",
            );

            builder.add_str(&format!(
                "keys.append(selector!(\"{}\"));",
                event.split_whitespace().next().unwrap()
            ));

            builder.add_str(&format!(
                "
                starknet::Event::append_keys_and_data(@{event}, ref keys, ref data);",
                event = event
            ));

            builder.add_str("\n            ");
            builder.add_node(world.as_syntax_node());
            builder.add_str(".emit(keys, data.span());");

            builder.add_str("}");
        }

        builder.add_str("}");

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "emit_inline_macro".into(),
                content: builder.code,
                code_mappings: builder.code_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

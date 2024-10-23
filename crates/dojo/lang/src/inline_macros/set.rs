use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, MacroPluginMetadata, NamedPlugin, PluginDiagnostic,
    PluginGeneratedFile,
};
use cairo_lang_defs::plugin_utils::unsupported_bracket_diagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::{ast, TypedStablePtr, TypedSyntaxNode};

use super::unsupported_arg_diagnostic;

#[derive(Debug, Default)]
pub struct SetMacro;

impl NamedPlugin for SetMacro {
    const NAME: &'static str = "set";
    // Parents of set!()
    // -----------------
    // StatementExpr
    // StatementList
    // ExprBlock
    // FunctionWithBody
    // ImplItemList
    // ImplBody
    // ItemImpl
    // ItemList
    // ModuleBody
    // ItemModule
    // ItemList
    // SyntaxFile
}

impl InlineMacroExprPlugin for SetMacro {
    fn generate_code(
        &self,
        db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
        syntax: &ast::ExprInlineMacro,
        _metadata: &MacroPluginMetadata<'_>,
    ) -> InlinePluginResult {
        let ast::WrappedArgList::ParenthesizedArgList(arg_list) = syntax.arguments(db) else {
            return unsupported_bracket_diagnostic(db, syntax);
        };
        let mut builder = PatchBuilder::new(db, syntax);
        builder.add_str("{");

        let args = arg_list.arguments(db).elements(db);

        if args.len() != 2 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: arg_list.arguments(db).stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \"(world, (models,))\"".to_string(),
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
                bundle.push(syntax_node.get_text(db));
            }
            ast::Expr::Tuple(list) => {
                list.expressions(db)
                    .elements(db)
                    .into_iter()
                    .for_each(|expr| {
                        let syntax_node = expr.as_syntax_node();
                        bundle.push(syntax_node.get_text(db));
                    })
            }
            ast::Expr::StructCtorCall(ctor) => {
                let syntax_node = ctor.as_syntax_node();
                bundle.push(syntax_node.get_text(db));
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

        for entity in bundle {
            builder.add_str(&format!(
                "
                dojo::model::ModelStore::set({}, @{});
                ",
                world.as_syntax_node().get_text(db),
                entity,
            ));
        }
        builder.add_str("}");

        let (code, code_mappings) = builder.build();

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "set_inline_macro".into(),
                content: code,
                code_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

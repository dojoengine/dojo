use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, MacroPluginMetadata, NamedPlugin, PluginDiagnostic,
    PluginGeneratedFile,
};
use cairo_lang_defs::plugin_utils::unsupported_bracket_diagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::Expr;
use cairo_lang_syntax::node::{ast, TypedStablePtr, TypedSyntaxNode};
use itertools::Itertools;

use super::{extract_models, unsupported_arg_diagnostic, CAIRO_ERR_MSG_LEN};

#[derive(Debug, Default)]
pub struct GetMacro;

impl NamedPlugin for GetMacro {
    const NAME: &'static str = "get";
}

impl InlineMacroExprPlugin for GetMacro {
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
        builder.add_str("{\n");

        let args = arg_list.arguments(db).elements(db);

        if args.len() != 3 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: syntax.stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \"get!(world, keys, (models,))\""
                        .to_string(),
                    severity: Severity::Error,
                }],
            };
        }

        let world = &args[0];

        let ast::ArgClause::Unnamed(keys) = args[1].arg_clause(db) else {
            return unsupported_arg_diagnostic(db, syntax);
        };

        let ast::ArgClause::Unnamed(models) = args[2].arg_clause(db) else {
            return unsupported_arg_diagnostic(db, syntax);
        };
        let models = match extract_models(db, &models.value(db)) {
            Ok(models) => models,
            Err(diagnostic) => {
                return InlinePluginResult {
                    code: None,
                    diagnostics: vec![diagnostic],
                };
            }
        };

        if models.is_empty() {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: syntax.stable_ptr().untyped(),
                    message: "Model types cannot be empty".to_string(),
                    severity: Severity::Error,
                }],
            };
        }

        let args = match keys.value(db) {
            Expr::Literal(literal) => format!("({})", literal.as_syntax_node().get_text(db)),
            _ => keys.as_syntax_node().get_text(db),
        };

        for model in &models {
            let mut lookup_err_msg = format!("{} not found", model.to_string());
            lookup_err_msg.truncate(CAIRO_ERR_MSG_LEN);

            builder.add_str(&format!(
                "let __{model}: {model} = dojo::model::ModelStore::get(@{}, {});\n",
                world.as_syntax_node().get_text(db),
                args,
            ));
        }
        builder.add_str(&format!(
            "({})}}",
            models.iter().map(|c| format!("__{c}")).join(",")
        ));

        let (code, code_mappings) = builder.build();

        crate::debug_expand(&format!("GET MACRO: {args}"), &code);

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "get_inline_macro".into(),
                content: code,
                code_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

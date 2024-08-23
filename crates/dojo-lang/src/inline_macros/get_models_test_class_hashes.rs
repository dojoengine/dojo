use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, MacroPluginMetadata, NamedPlugin, PluginDiagnostic,
    PluginGeneratedFile,
};
use cairo_lang_defs::plugin_utils::unsupported_bracket_diagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, TypedStablePtr, TypedSyntaxNode};

use super::unsupported_arg_diagnostic;
use super::utils::load_manifest_models_and_namespaces;

#[derive(Debug, Default)]
pub struct GetModelsTestClassHashes;

impl NamedPlugin for GetModelsTestClassHashes {
    const NAME: &'static str = "get_models_test_class_hashes";
}

impl InlineMacroExprPlugin for GetModelsTestClassHashes {
    fn generate_code(
        &self,
        db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
        syntax: &ast::ExprInlineMacro,
        metadata: &MacroPluginMetadata<'_>,
    ) -> InlinePluginResult {
        let ast::WrappedArgList::ParenthesizedArgList(arg_list) = syntax.arguments(db) else {
            return unsupported_bracket_diagnostic(db, syntax);
        };

        let args = arg_list.arguments(db).elements(db);

        if args.len() > 1 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: syntax.stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \
                              \"get_models_test_class_hashes!([\"ns1\", \"ns2\")]\" or \
                              \"get_models_test_class_hashes!()\"."
                        .to_string(),
                    severity: Severity::Error,
                }],
            };
        }

        let whitelisted_namespaces = if args.len() == 1 {
            let ast::ArgClause::Unnamed(expected_array) = args[0].arg_clause(db) else {
                return unsupported_arg_diagnostic(db, syntax);
            };

            match extract_namespaces(db, &expected_array.value(db)) {
                Ok(namespaces) => namespaces,
                Err(e) => {
                    return InlinePluginResult { code: None, diagnostics: vec![e] };
                }
            }
        } else {
            vec![]
        };

        let (_namespaces, models) =
            match load_manifest_models_and_namespaces(metadata.cfg_set, &whitelisted_namespaces) {
                Ok((namespaces, models)) => (namespaces, models),
                Err(_e) => {
                    return InlinePluginResult {
                        code: None,
                        diagnostics: vec![PluginDiagnostic {
                            stable_ptr: syntax.stable_ptr().untyped(),
                            message: "Failed to load models and namespaces, ensure you have run \
                                      `sozo build` first."
                                .to_string(),
                            severity: Severity::Error,
                        }],
                    };
                }
            };

        let mut builder = PatchBuilder::new(db, syntax);

        // Use the TEST_CLASS_HASH for each model, which is already a qualified path, no `use`
        // required.
        builder.add_str(&format!(
            "[{}].span()",
            models
                .iter()
                .map(|m| format!("{}::TEST_CLASS_HASH", m))
                .collect::<Vec<String>>()
                .join(", ")
        ));

        let (code, code_mappings) = builder.build();

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "get_models_test_class_hashes_macro".into(),
                content: code,
                code_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

/// Extracts the namespaces from a fixed size array of strings.
fn extract_namespaces(
    db: &dyn SyntaxGroup,
    expression: &ast::Expr,
) -> Result<Vec<String>, PluginDiagnostic> {
    let mut namespaces = vec![];

    match expression {
        ast::Expr::FixedSizeArray(array) => {
            for element in array.exprs(db).elements(db) {
                if let ast::Expr::String(string_literal) = element {
                    namespaces.push(string_literal.as_syntax_node().get_text(db).replace('\"', ""));
                } else {
                    return Err(PluginDiagnostic {
                        stable_ptr: element.stable_ptr().untyped(),
                        message: "Expected a string literal".to_string(),
                        severity: Severity::Error,
                    });
                }
            }
        }
        _ => {
            return Err(PluginDiagnostic {
                stable_ptr: expression.stable_ptr().untyped(),
                message: "The list of namespaces should be a fixed size array of strings."
                    .to_string(),
                severity: Severity::Error,
            });
        }
    }

    Ok(namespaces)
}

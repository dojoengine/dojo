use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, MacroPluginMetadata, NamedPlugin, PluginDiagnostic,
    PluginGeneratedFile,
};
use cairo_lang_defs::plugin_utils::unsupported_bracket_diagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::{ast, TypedStablePtr, TypedSyntaxNode};

use super::utils::load_manifest_models_and_namespaces;

#[derive(Debug, Default)]
pub struct SpawnTestWorldFull;

impl NamedPlugin for SpawnTestWorldFull {
    const NAME: &'static str = "spawn_test_world_full";
}

impl InlineMacroExprPlugin for SpawnTestWorldFull {
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

        if !args.is_empty() {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: syntax.stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \"spawn_test_world_full!()\" with no \
                              arguments."
                        .to_string(),
                    severity: Severity::Error,
                }],
            };
        }

        let (namespaces, models) = match load_manifest_models_and_namespaces(metadata.cfg_set, &[])
        {
            Ok((namespaces, models)) => (namespaces, models),
            Err(_e) => {
                return InlinePluginResult {
                    code: None,
                    diagnostics: vec![PluginDiagnostic {
                        stable_ptr: syntax.stable_ptr().untyped(),
                        message: "Failed to load models and namespaces, ensure you have run `sozo \
                                  build` first."
                            .to_string(),
                        severity: Severity::Error,
                    }],
                };
            }
        };

        let mut builder = PatchBuilder::new(db, syntax);

        builder.add_str(&format!(
            "dojo::utils::test::spawn_test_world([{}].span(), [{}].span())",
            namespaces.iter().map(|n| format!("\"{}\"", n)).collect::<Vec<String>>().join(", "),
            models
                .iter()
                .map(|m| format!("{}::TEST_CLASS_HASH", m))
                .collect::<Vec<String>>()
                .join(", ")
        ));

        let (code, code_mappings) = builder.build();

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "spawn_test_world_full_macro".into(),
                content: code,
                code_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

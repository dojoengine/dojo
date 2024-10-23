use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, MacroPluginMetadata, NamedPlugin, PluginDiagnostic,
    PluginGeneratedFile,
};
use cairo_lang_defs::plugin_utils::unsupported_bracket_diagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::{ast, TypedStablePtr, TypedSyntaxNode};
use dojo_types::naming;

#[derive(Debug, Default)]
pub struct SelectorFromTagMacro;

impl NamedPlugin for SelectorFromTagMacro {
    const NAME: &'static str = "selector_from_tag";
}

impl InlineMacroExprPlugin for SelectorFromTagMacro {
    fn generate_code(
        &self,
        db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
        syntax: &ast::ExprInlineMacro,
        _metadata: &MacroPluginMetadata<'_>,
    ) -> InlinePluginResult {
        let ast::WrappedArgList::ParenthesizedArgList(arg_list) = syntax.arguments(db) else {
            return unsupported_bracket_diagnostic(db, syntax);
        };

        let args = arg_list.arguments(db).elements(db);

        if args.len() != 1 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: syntax.stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \"selector_from_tag!(\"tag\")\""
                        .to_string(),
                    severity: Severity::Error,
                }],
            };
        }

        let tag = &args[0].as_syntax_node().get_text(db).replace('\"', "");

        if !naming::is_valid_tag(tag) {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: syntax.stable_ptr().untyped(),
                    message: "Invalid tag. Tag must be in the format of `namespace-name`."
                        .to_string(),
                    severity: Severity::Error,
                }],
            };
        }

        let selector = naming::compute_selector_from_tag(tag);

        let mut builder = PatchBuilder::new(db, syntax);
        builder.add_str(&format!("{:#64x}", selector));

        let (code, code_mappings) = builder.build();

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "selector_from_tag_macro".into(),
                content: code,
                code_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

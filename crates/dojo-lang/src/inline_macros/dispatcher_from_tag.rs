use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, MacroPluginMetadata, NamedPlugin, PluginDiagnostic,
    PluginGeneratedFile,
};
use cairo_lang_defs::plugin_utils::unsupported_bracket_diagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::{ast, TypedStablePtr, TypedSyntaxNode};
use dojo_world::contracts::naming;

use super::utils::find_interface_path;

#[derive(Debug, Default)]
pub struct DispatcherFromTagMacro;

impl NamedPlugin for DispatcherFromTagMacro {
    const NAME: &'static str = "dispatcher_from_tag";
}

impl InlineMacroExprPlugin for DispatcherFromTagMacro {
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

        if args.len() != 2 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: syntax.stable_ptr().untyped(),
                    message: "Invalid arguments. Expected dispatcher_from_tag!(\"tag\", contract_address)"
                        .to_string(),
                    severity: Severity::Error,
                }],
            };
        }

        let tag = &args[0].as_syntax_node().get_text(db).replace('\"', "");
        let contract_address = args[1].as_syntax_node().get_text(db);

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

        // read the interface path from the manifest and generate a dispatcher:
        // <interface_path>Dispatcher { contract_address };
        let interface_path = match find_interface_path(metadata.cfg_set, tag) {
            Ok(interface_path) => interface_path,
            Err(_e) => {
                return InlinePluginResult {
                    code: None,
                    diagnostics: vec![PluginDiagnostic {
                        stable_ptr: syntax.stable_ptr().untyped(),
                        message: format!("Failed to find the interface path of `{tag}`"),
                        severity: Severity::Error,
                    }],
                };
            }
        };

        let mut builder = PatchBuilder::new(db, syntax);
        builder.add_str(&format!(
            "{interface_path}Dispatcher {{ contract_address: {contract_address}}}",
        ));

        let (code, code_mappings) = builder.build();

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "dispatcher_from_tag_macro".into(),
                content: code,
                code_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

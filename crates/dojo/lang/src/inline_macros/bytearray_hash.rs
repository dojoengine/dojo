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
pub struct BytearrayHashMacro;

impl NamedPlugin for BytearrayHashMacro {
    const NAME: &'static str = "bytearray_hash";
}

impl InlineMacroExprPlugin for BytearrayHashMacro {
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
                    message: "Invalid arguments. Expected \"bytearray_hash!(\"long string\")\""
                        .to_string(),
                    severity: Severity::Error,
                }],
            };
        }

        let bytearray = &args[0].as_syntax_node().get_text(db).replace('\"', "");

        let bytearray_hash = naming::compute_bytearray_hash(bytearray);

        let mut builder = PatchBuilder::new(db, syntax);
        builder.add_str(&format!("{:#64x}", bytearray_hash));

        let (code, code_mappings) = builder.build();

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "bytearray_hash_macro".into(),
                content: code,
                code_mappings,
                diagnostics_note: None,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

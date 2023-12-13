use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, NamedPlugin, PluginDiagnostic, PluginGeneratedFile,
};
use cairo_lang_semantic::inline_macros::unsupported_bracket_diagnostic;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};

#[derive(Debug, Default)]
pub struct RandomMacro;

impl NamedPlugin for RandomMacro {
    const NAME: &'static str = "random";
}

impl InlineMacroExprPlugin for RandomMacro {
    fn generate_code(
        &self,
        db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
        syntax: &ast::ExprInlineMacro,
    ) -> InlinePluginResult {
        let ast::WrappedArgList::ParenthesizedArgList(arg_list) = syntax.arguments(db) else {
            return unsupported_bracket_diagnostic(db, syntax);
        };
        let mut builder = PatchBuilder::new(db);

        let args = arg_list.arguments(db).elements(db);

        if args.len() != 1 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: arg_list.arguments(db).stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \"random!(seed)\"".to_string(),
                }],
            };
        }

        let seed = &args[0];

        // call get_random syscall
        builder.add_str(
            "{
                let (hash, proof, pub) = starknet::syscalls::get_random(",
        );
        builder.add_node(seed.as_syntax_node());
        builder.add_str(");");

        builder.add_str("\n            hash");
        builder.add_str("\n}");

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "random_inline_macro".into(),
                content: builder.code,
                diagnostics_mappings: builder.diagnostics_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, NamedPlugin, PluginDiagnostic, PluginGeneratedFile,
};
use cairo_lang_semantic::inline_macros::unsupported_bracket_diagnostic;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};

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
        builder.add_str(
            "{
                let mut keys = Default::<core::array::Array>::default();
                let mut data = Default::<core::array::Array>::default();",
        );

        let args = arg_list.arguments(db).elements(db);

        if args.len() != 2 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: arg_list.arguments(db).stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \"emit!(world, event)\"".to_string(),
                }],
            };
        }

        let world = &args[0];
        let event = &args[1];

        builder.add_str(
            "\n            starknet::Event::append_keys_and_data(@core::traits::Into::<_, \
             Event>::into(",
        );
        builder.add_node(event.as_syntax_node());
        builder.add_str("), ref keys, ref data);");

        builder.add_str("\n            ");
        builder.add_node(world.as_syntax_node());
        builder.add_str(".emit(keys, data.span());");
        builder.add_str("}");

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "emit_inline_macro".into(),
                content: builder.code,
                diagnostics_mappings: builder.diagnostics_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

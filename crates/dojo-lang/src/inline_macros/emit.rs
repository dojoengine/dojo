use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_syntax::node::TypedSyntaxNode;

use crate::inline_macro_plugin::{InlineMacro, InlineMacroExpanderData};

pub struct EmitMacro;
impl InlineMacro for EmitMacro {
    fn append_macro_code(
        &self,
        macro_expander_data: &mut InlineMacroExpanderData,
        db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
        macro_arguments: &cairo_lang_syntax::node::ast::ExprList,
    ) {
        let args = macro_arguments.elements(db);

        if args.len() != 2 {
            macro_expander_data.diagnostics.push(PluginDiagnostic {
                message: "Invalid arguments. Expected \"emit!(world, event)\"".to_string(),
                stable_ptr: macro_arguments.as_syntax_node().stable_ptr(),
            });
            return;
        }

        let world = &args[0];
        let event = &args[1];
        let expanded_code = format!(
            "{{
                let mut keys = Default::<array::Array>::default();
                let mut data = Default::<array::Array>::default();
                starknet::Event::append_keys_and_data(@traits::Into::<_, Event>::into({}), ref \
             keys, ref data);
                {}.emit(keys, data.span());
            }}",
            event.as_syntax_node().get_text(db),
            world.as_syntax_node().get_text(db),
        );
        macro_expander_data.result_code.push_str(&expanded_code);
        macro_expander_data.code_changed = true;
    }

    fn is_bracket_type_allowed(
        &self,
        db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
        macro_ast: &cairo_lang_syntax::node::ast::ExprInlineMacro,
    ) -> bool {
        matches!(
            macro_ast.arguments(db),
            cairo_lang_syntax::node::ast::WrappedExprList::ParenthesizedExprList(_)
        )
    }
}

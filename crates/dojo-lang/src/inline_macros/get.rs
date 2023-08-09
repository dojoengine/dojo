use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_syntax::node::ast::Expr;
use cairo_lang_syntax::node::TypedSyntaxNode;
use itertools::Itertools;

use super::{extract_components, CAIRO_ERR_MSG_LEN};
use crate::inline_macro_plugin::{InlineMacro, InlineMacroExpanderData};

pub struct GetMacro;
impl InlineMacro for GetMacro {
    fn append_macro_code(
        &self,
        macro_expander_data: &mut InlineMacroExpanderData,
        db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
        macro_arguments: &cairo_lang_syntax::node::ast::ExprList,
    ) {
        let args = macro_arguments.elements(db);

        if args.len() != 3 {
            macro_expander_data.diagnostics.push(PluginDiagnostic {
                message: "Invalid arguments. Expected \"get!(world, keys, (components,))\""
                    .to_string(),
                stable_ptr: macro_arguments.as_syntax_node().stable_ptr(),
            });
            return;
        }

        let world = &args[0];
        let keys = &args[1];
        let components = extract_components(db, &args[2]);

        if components.is_empty() {
            macro_expander_data.diagnostics.push(PluginDiagnostic {
                message: "Component types cannot be empty".to_string(),
                stable_ptr: macro_arguments.as_syntax_node().stable_ptr(),
            });
            return;
        }

        let args = match keys {
            Expr::Literal(literal) => format!("({})", literal.as_syntax_node().get_text(db)),
            _ => keys.as_syntax_node().get_text(db),
        };

        let mut expanded_code = format!(
            "{{
            let mut __get_macro_keys__ = array::ArrayTrait::new();
            serde::Serde::serialize(@{args}, ref __get_macro_keys__);
            let __get_macro_keys__ = array::ArrayTrait::span(@__get_macro_keys__);"
        );

        for component in &components {
            let mut lookup_err_msg = format!("{} not found", component.to_string());
            lookup_err_msg.truncate(CAIRO_ERR_MSG_LEN);
            let mut deser_err_msg = format!("{} failed to deserialize", component.to_string());
            deser_err_msg.truncate(CAIRO_ERR_MSG_LEN);

            expanded_code.push_str(&format!(
                "\n            let __{component}_values__ = {}.entity('{component}', \
                 __get_macro_keys__, 0_u8, dojo::SerdeLen::<{component}>::len());
                 let mut __{component}_component__ = array::ArrayTrait::new();
                 array::serialize_array_helper(__get_macro_keys__, ref __{component}_component__);
                 array::serialize_array_helper(__{component}_values__, ref \
                 __{component}_component__);
                 let mut __{component}_component_span__ = \
                 array::ArrayTrait::span(@__{component}_component__);
                   let __{component} = \
                 option::OptionTrait::expect(serde::Serde::<{component}>::deserialize(
                       ref __{component}_component_span__
                   ), '{deser_err_msg}');",
                world.as_syntax_node().get_text(db),
            ));
        }
        expanded_code.push_str(
            format!(
                "({})
        }}",
                components.iter().map(|c| format!("__{c}")).join(",")
            )
            .as_str(),
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

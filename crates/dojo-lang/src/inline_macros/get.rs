use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, PluginDiagnostic, PluginGeneratedFile,
};
use cairo_lang_semantic::inline_macros::unsupported_bracket_diagnostic;
use cairo_lang_syntax::node::ast::Expr;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use itertools::Itertools;

use super::{extract_components, unsupported_arg_diagnostic, CAIRO_ERR_MSG_LEN};

#[derive(Debug)]
pub struct GetMacro;
impl GetMacro {
    pub const NAME: &'static str = "get";
}
impl InlineMacroExprPlugin for GetMacro {
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
                let mut __get_macro_keys__ = array::ArrayTrait::new();",
        );

        let args = arg_list.args(db).elements(db);

        if args.len() != 3 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: syntax.stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \"get!(world, keys, (components,))\""
                        .to_string(),
                }],
            };
        }

        let world = &args[0];

        let ast::ArgClause::Unnamed(keys) = args[1].arg_clause(db) else {
            return unsupported_arg_diagnostic(db, syntax);
        };

        let ast::ArgClause::Unnamed(components) = args[2].arg_clause(db) else {
            return unsupported_arg_diagnostic(db, syntax);
        };
        let components = match extract_components(db, &components.value(db)) {
            Ok(components) => components,
            Err(diagnostic) => {
                return InlinePluginResult { code: None, diagnostics: vec![diagnostic] };
            }
        };

        if components.is_empty() {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: syntax.stable_ptr().untyped(),
                    message: "Component types cannot be empty".to_string(),
                }],
            };
        }

        let args = match keys.value(db) {
            Expr::Literal(literal) => format!("({})", literal.as_syntax_node().get_text(db)),
            _ => keys.as_syntax_node().get_text(db),
        };

        builder.add_str(&format!(
            "serde::Serde::serialize(@{args}, ref __get_macro_keys__);
            let __get_macro_keys__ = array::ArrayTrait::span(@__get_macro_keys__);"
        ));

        for component in &components {
            let mut lookup_err_msg = format!("{} not found", component.to_string());
            lookup_err_msg.truncate(CAIRO_ERR_MSG_LEN);
            let mut deser_err_msg = format!("{} failed to deserialize", component.to_string());
            deser_err_msg.truncate(CAIRO_ERR_MSG_LEN);

            builder.add_str(&format!(
                "\n            let mut __{component}_values__ = {}.entity('{component}', \
                 __get_macro_keys__, 0_u8, dojo::StorageLayout::<{component}>::size());
                 let __{component} = \
                 option::OptionTrait::expect(dojo::component::Component::<{component}>::unpack(ref \
                 __{component}_values__), '{deser_err_msg}');\n",
                world.as_syntax_node().get_text(db),
            ));
        }
        builder.add_str(&format!(
            "({})
        }}",
            components.iter().map(|c| format!("__{c}")).join(",")
        ));

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "get_inline_macro".into(),
                content: builder.code,
                diagnostics_mappings: builder.diagnostics_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

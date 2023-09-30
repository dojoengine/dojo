use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, PluginDiagnostic, PluginGeneratedFile,
};
use cairo_lang_semantic::inline_macros::unsupported_bracket_diagnostic;
use cairo_lang_syntax::node::ast::Expr;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use itertools::Itertools;

use super::{extract_models, unsupported_arg_diagnostic, CAIRO_ERR_MSG_LEN};

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
                let mut __get_macro_keys__ = array::ArrayTrait::new();\n",
        );

        let args = arg_list.args(db).elements(db);

        if args.len() != 3 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: syntax.stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \"get!(world, keys, (models,))\""
                        .to_string(),
                }],
            };
        }

        let world = &args[0];

        let ast::ArgClause::Unnamed(keys) = args[1].arg_clause(db) else {
            return unsupported_arg_diagnostic(db, syntax);
        };

        let ast::ArgClause::Unnamed(models) = args[2].arg_clause(db) else {
            return unsupported_arg_diagnostic(db, syntax);
        };
        let models = match extract_models(db, &models.value(db)) {
            Ok(models) => models,
            Err(diagnostic) => {
                return InlinePluginResult { code: None, diagnostics: vec![diagnostic] };
            }
        };

        if models.is_empty() {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: syntax.stable_ptr().untyped(),
                    message: "Model types cannot be empty".to_string(),
                }],
            };
        }

        let args = match keys.value(db) {
            Expr::Literal(literal) => format!("({})", literal.as_syntax_node().get_text(db)),
            _ => keys.as_syntax_node().get_text(db),
        };

        builder.add_str(&format!(
            "serde::Serde::serialize(@{args}, ref __get_macro_keys__);
            let __get_macro_keys__ = array::ArrayTrait::span(@__get_macro_keys__);\n"
        ));

        for model in &models {
            let mut lookup_err_msg = format!("{} not found", model.to_string());
            lookup_err_msg.truncate(CAIRO_ERR_MSG_LEN);
            let mut deser_err_msg = format!("{} failed to deserialize", model.to_string());
            deser_err_msg.truncate(CAIRO_ERR_MSG_LEN);

            builder.add_str(&format!(
                "\n            let mut __{model}_layout__ = array::ArrayTrait::new();
                 dojo::database::schema::SchemaIntrospection::<{model}>::layout(ref \
                 __{model}_layout__);
                 let mut __{model}_layout_clone__ = __{model}_layout__.clone();
                 let mut __{model}_layout_span__ = array::ArrayTrait::span(@__{model}_layout__);
                 let mut __{model}_layout_clone_span__ = \
                 array::ArrayTrait::span(@__{model}_layout_clone__);
                 let mut __{model}_values__ = {}.entity('{model}', __get_macro_keys__, 0_u8,
                 dojo::packing::calculate_packed_size(ref __{model}_layout_clone_span__),
                 __{model}_layout_span__);
                 let mut __{model}_model__ = array::ArrayTrait::new();
                 array::serialize_array_helper(__get_macro_keys__, ref __{model}_model__);
                 array::serialize_array_helper(__{model}_values__, ref __{model}_model__);
                 let mut __{model}_model_span__ = array::ArrayTrait::span(@__{model}_model__);
                 let __{model} = option::OptionTrait::expect(serde::Serde::<{model}>::deserialize(
                    ref __{model}_model_span__
                ), '{deser_err_msg}');\n",
                world.as_syntax_node().get_text(db),
            ));
        }
        builder.add_str(&format!(
            "({})
        }}",
            models.iter().map(|c| format!("__{c}")).join(",")
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

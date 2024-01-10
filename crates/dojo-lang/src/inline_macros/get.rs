use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, NamedPlugin, PluginDiagnostic, PluginGeneratedFile,
};
use cairo_lang_semantic::inline_macros::unsupported_bracket_diagnostic;
use cairo_lang_syntax::node::ast::{Expr, ItemModule};
use cairo_lang_syntax::node::kind::SyntaxKind;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use itertools::Itertools;

use super::utils::{parent_of_kind, SYSTEM_READS};
use super::{extract_models, unsupported_arg_diagnostic, CAIRO_ERR_MSG_LEN};

#[derive(Debug, Default)]
pub struct GetMacro;

impl NamedPlugin for GetMacro {
    const NAME: &'static str = "get";
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
                let mut __get_macro_keys__ = core::array::ArrayTrait::new();\n",
        );

        let args = arg_list.arguments(db).elements(db);

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
            "core::serde::Serde::serialize(@{args}, ref __get_macro_keys__);
            let __get_macro_keys__ = core::array::ArrayTrait::span(@__get_macro_keys__);\n"
        ));

        let mut system_reads = SYSTEM_READS.lock().unwrap();

        let module_syntax_node =
            parent_of_kind(db, &syntax.as_syntax_node(), SyntaxKind::ItemModule);
        let module_name = if let Some(module_syntax_node) = &module_syntax_node {
            let mod_ast = ItemModule::from_syntax_node(db, module_syntax_node.clone());
            mod_ast.name(db).as_syntax_node().get_text_without_trivia(db)
        } else {
            "".into()
        };

        for model in &models {
            if !module_name.is_empty() {
                if system_reads.get(&module_name).is_none() {
                    system_reads.insert(module_name.clone(), vec![model.to_string()]);
                } else {
                    system_reads.get_mut(&module_name).unwrap().push(model.to_string());
                }
            }
            let mut lookup_err_msg = format!("{} not found", model.to_string());
            lookup_err_msg.truncate(CAIRO_ERR_MSG_LEN);
            // Currently, the main reason to have a deserialization to fail is by having
            // the user providing the wrong keys length, which causes an invalid offset
            // in the model deserialization.
            let deser_err_msg = format!(
                "\"Model `{}`: deserialization failed. Ensure the length of the keys tuple is \
                 matching the number of #[key] fields in the model struct.\"",
                model.to_string()
            );

            builder.add_str(&format!(
                "\n            let mut __{model}_layout__ = core::array::ArrayTrait::new();
                 dojo::database::introspect::Introspect::<{model}>::layout(ref __{model}_layout__);
                 let mut __{model}_layout_clone__ = __{model}_layout__.clone();
                 let mut __{model}_layout_span__ = \
                 core::array::ArrayTrait::span(@__{model}_layout__);
                 let mut __{model}_layout_clone_span__ = \
                 core::array::ArrayTrait::span(@__{model}_layout_clone__);
                 let mut __{model}_values__ = {}.entity('{model}', __get_macro_keys__, 0_u8,
                 dojo::packing::calculate_packed_size(ref __{model}_layout_clone_span__),
                 __{model}_layout_span__);
                 let mut __{model}_model__ = core::array::ArrayTrait::new();
                 core::array::serialize_array_helper(__get_macro_keys__, ref __{model}_model__);
                 core::array::serialize_array_helper(__{model}_values__, ref __{model}_model__);
                 let mut __{model}_model_span__ = \
                 core::array::ArrayTrait::span(@__{model}_model__);
                 let __{model} = core::serde::Serde::<{model}>::deserialize(
                    ref __{model}_model_span__
                ); if core::option::OptionTrait::<{model}>::is_none(@__{model}) {{ \
                 panic!({deser_err_msg}); }}; let __{model} = \
                 core::option::OptionTrait::<{model}>::unwrap(__{model});\n",
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

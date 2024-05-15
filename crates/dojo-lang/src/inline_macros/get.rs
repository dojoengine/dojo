use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, NamedPlugin, PluginDiagnostic, PluginGeneratedFile,
};
use cairo_lang_diagnostics::Severity;
use cairo_lang_defs::plugin_utils::unsupported_bracket_diagnostic;
use cairo_lang_syntax::node::ast::{Expr, ItemModule};
use cairo_lang_syntax::node::kind::SyntaxKind;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode, TypedStablePtr};
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
        let mut builder = PatchBuilder::new(db, syntax);
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
                    severity: Severity::Error,
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
                    severity: Severity::Error,
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

            builder.add_str(&format!(
                "\n
                let __{model}_layout__ = dojo::model::Model::<{model}>::layout();
                let __{model}: {model} = dojo::model::Model::entity({}, __get_macro_keys__, \
                 __{model}_layout__);\n",
                world.as_syntax_node().get_text(db),
            ));
        }
        builder.add_str(&format!(
            "({})
        }}",
            models.iter().map(|c| format!("__{c}")).join(",")
        ));

        let (code, code_mappings) = builder.build();

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "get_inline_macro".into(),
                content: code,
                code_mappings: code_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

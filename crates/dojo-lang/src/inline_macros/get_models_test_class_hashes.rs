use std::collections::HashSet;

use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    InlineMacroExprPlugin, InlinePluginResult, MacroPluginMetadata, NamedPlugin, PluginDiagnostic,
    PluginGeneratedFile,
};
use cairo_lang_defs::plugin_utils::unsupported_bracket_diagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_filesystem::cfg::CfgSet;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, TypedStablePtr, TypedSyntaxNode};
use camino::Utf8PathBuf;
use dojo_world::config::namespace_config::DOJO_MANIFESTS_DIR_CFG_KEY;
use dojo_world::contracts::naming;
use dojo_world::manifest::BaseManifest;

use super::unsupported_arg_diagnostic;

#[derive(Debug, Default)]
pub struct GetModelsTestClassHashes;

impl NamedPlugin for GetModelsTestClassHashes {
    const NAME: &'static str = "get_models_test_class_hashes";
}

impl InlineMacroExprPlugin for GetModelsTestClassHashes {
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

        if args.len() > 1 {
            return InlinePluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    stable_ptr: syntax.stable_ptr().untyped(),
                    message: "Invalid arguments. Expected \
                              \"get_models_test_class_hashes!([\"ns1\", \"ns2\")]\" or \
                              \"get_models_test_class_hashes!()\""
                        .to_string(),
                    severity: Severity::Error,
                }],
            };
        }

        let whitelisted_namespaces = if args.len() == 1 {
            let ast::ArgClause::Unnamed(expected_array) = args[0].arg_clause(db) else {
                return unsupported_arg_diagnostic(db, syntax);
            };

            match extract_namespaces(db, &expected_array.value(db)) {
                Ok(namespaces) => namespaces,
                Err(e) => {
                    return InlinePluginResult {
                        code: None,
                        diagnostics: vec![PluginDiagnostic {
                            stable_ptr: syntax.stable_ptr().untyped(),
                            message: format!("Error extracting namespaces: {:?}", e),
                            severity: Severity::Error,
                        }],
                    };
                }
            }
        } else {
            vec![]
        };

        let (_namespaces, models) =
            match load_manifest_models_and_namespaces(metadata.cfg_set, whitelisted_namespaces) {
                Ok((namespaces, models)) => (namespaces, models),
                Err(e) => {
                    return InlinePluginResult {
                        code: None,
                        diagnostics: vec![PluginDiagnostic {
                            stable_ptr: syntax.stable_ptr().untyped(),
                            message: format!("Failed to load models and namespaces: {}", e),
                            severity: Severity::Error,
                        }],
                    };
                }
            };

        let mut builder = PatchBuilder::new(db, syntax);

        // Use the TEST_CLASS_HASH for each model, which is already a qualified path, no `use`
        // required.
        builder.add_str(&format!(
            "[{}].span()",
            models
                .iter()
                .map(|m| format!("{}::TEST_CLASS_HASH", m))
                .collect::<Vec<String>>()
                .join(", ")
        ));

        let (code, code_mappings) = builder.build();

        InlinePluginResult {
            code: Some(PluginGeneratedFile {
                name: "get_models_test_class_hashes_macro".into(),
                content: code,
                code_mappings,
                aux_data: None,
            }),
            diagnostics: vec![],
        }
    }
}

/// Extracts the namespaces from a fixed size array of strings.
fn extract_namespaces(
    db: &dyn SyntaxGroup,
    expression: &ast::Expr,
) -> Result<Vec<String>, PluginDiagnostic> {
    let mut namespaces = vec![];

    match expression {
        ast::Expr::FixedSizeArray(array) => {
            for element in array.exprs(db).elements(db) {
                if let ast::Expr::String(string_literal) = element {
                    namespaces.push(string_literal.as_syntax_node().get_text(db).replace('\"', ""));
                } else {
                    return Err(PluginDiagnostic {
                        stable_ptr: element.stable_ptr().untyped(),
                        message: "Expected a string literal".to_string(),
                        severity: Severity::Error,
                    });
                }
            }
        }
        _ => {
            return Err(PluginDiagnostic {
                stable_ptr: expression.stable_ptr().untyped(),
                message: format!(
                    "The list of namespaces should be a fixed size array of strings, found: {}",
                    expression.as_syntax_node().get_text(db)
                ),
                severity: Severity::Error,
            });
        }
    }

    Ok(namespaces)
}

/// Reads all the models and namespaces from base manifests files.
fn load_manifest_models_and_namespaces(
    cfg_set: &CfgSet,
    whitelisted_namespaces: Vec<String>,
) -> anyhow::Result<(Vec<String>, Vec<String>)> {
    let dojo_manifests_dir = get_dojo_manifests_dir(cfg_set.clone())?;

    let base_dir = dojo_manifests_dir.join("base");
    let base_abstract_manifest = BaseManifest::load_from_path(&base_dir)?;

    let mut models = HashSet::new();
    let mut namespaces = HashSet::new();

    for model in base_abstract_manifest.models {
        let qualified_path = model.inner.qualified_path;
        let namespace = naming::split_tag(&model.inner.tag)?.0;

        if !whitelisted_namespaces.is_empty() && !whitelisted_namespaces.contains(&namespace) {
            continue;
        }

        models.insert(qualified_path);
        namespaces.insert(namespace);
    }

    let models_vec: Vec<String> = models.into_iter().collect();
    let namespaces_vec: Vec<String> = namespaces.into_iter().collect();

    Ok((namespaces_vec, models_vec))
}

/// Gets the dojo_manifests_dir from the cfg_set.
fn get_dojo_manifests_dir(cfg_set: CfgSet) -> anyhow::Result<Utf8PathBuf> {
    for cfg in cfg_set.into_iter() {
        if cfg.key == DOJO_MANIFESTS_DIR_CFG_KEY {
            return Ok(Utf8PathBuf::from(cfg.value.unwrap().as_str().to_string()));
        }
    }

    Err(anyhow::anyhow!("dojo_manifests_dir not found"))
}

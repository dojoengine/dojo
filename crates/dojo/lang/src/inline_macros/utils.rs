use std::collections::HashSet;

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_filesystem::cfg::CfgSet;
use cairo_lang_syntax::node::ast::{self, ExprPath, ExprStructCtorCall};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::kind::SyntaxKind;
use cairo_lang_syntax::node::{SyntaxNode, TypedStablePtr, TypedSyntaxNode};
use camino::Utf8PathBuf;
use dojo_world::config::namespace_config::DOJO_MANIFESTS_DIR_CFG_KEY;
use dojo_world::contracts::naming;
use dojo_world::manifest::BaseManifest;

#[derive(Debug)]
pub enum SystemRWOpRecord {
    StructCtor(ExprStructCtorCall),
    Path(ExprPath),
}

pub fn parent_of_kind(
    db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
    target: &SyntaxNode,
    kind: SyntaxKind,
) -> Option<SyntaxNode> {
    let mut new_target = target.clone();
    while let Some(parent) = new_target.parent() {
        if kind == parent.kind(db) {
            return Some(parent);
        }
        new_target = parent;
    }
    None
}

/// Reads all the models and namespaces from base manifests files.
pub fn load_manifest_models_and_namespaces(
    cfg_set: &CfgSet,
    whitelisted_namespaces: &[String],
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
pub fn get_dojo_manifests_dir(cfg_set: CfgSet) -> anyhow::Result<Utf8PathBuf> {
    for cfg in cfg_set.into_iter() {
        if cfg.key == DOJO_MANIFESTS_DIR_CFG_KEY {
            return Ok(Utf8PathBuf::from(cfg.value.unwrap().as_str().to_string()));
        }
    }

    Err(anyhow::anyhow!("dojo_manifests_dir not found"))
}

/// Extracts the namespaces from a fixed size array of strings.
pub fn extract_namespaces(
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
                message: "The list of namespaces should be a fixed size array of strings."
                    .to_string(),
                severity: Severity::Error,
            });
        }
    }

    Ok(namespaces)
}

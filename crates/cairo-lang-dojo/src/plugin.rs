use std::collections::HashMap;
use std::sync::Arc;
use std::vec;

use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, GeneratedFileAuxData, MacroPlugin, PluginDiagnostic,
    PluginGeneratedFile, PluginResult,
};
use cairo_lang_diagnostics::DiagnosticEntry;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::patcher::{ModifiedNode, PatchBuilder, Patches, RewriteNode};
use cairo_lang_semantic::plugin::{
    AsDynGeneratedFileAuxData, AsDynMacroPlugin, DiagnosticMapper, DynDiagnosticMapper,
    PluginMappedDiagnostic, SemanticPlugin,
};
use cairo_lang_semantic::SemanticDiagnostic;
use cairo_lang_syntax::node::ast::MaybeModuleBody;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use indoc::formatdoc;

use crate::component::Component;
use crate::system::System;

const COMPONENT_ATTR: &str = "component";
const SYSTEM_ATTR: &str = "system";

/// The diagnostics remapper of the plugin.
#[derive(Debug, PartialEq, Eq)]
pub struct DiagnosticRemapper {
    patches: Patches,
}
impl GeneratedFileAuxData for DiagnosticRemapper {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn eq(&self, other: &dyn GeneratedFileAuxData) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<Self>() {
            self == other
        } else {
            false
        }
    }
}
impl AsDynGeneratedFileAuxData for DiagnosticRemapper {
    fn as_dyn_macro_token(&self) -> &(dyn GeneratedFileAuxData + 'static) {
        self
    }
}
impl DiagnosticMapper for DiagnosticRemapper {
    fn map_diag(
        &self,
        db: &(dyn SemanticGroup + 'static),
        diag: &dyn std::any::Any,
    ) -> Option<PluginMappedDiagnostic> {
        let Some(diag) = diag.downcast_ref::<SemanticDiagnostic>() else {return None;};
        let span = self
            .patches
            .translate(db.upcast(), diag.stable_location.diagnostic_location(db.upcast()).span)?;
        Some(PluginMappedDiagnostic { span, message: diag.format(db) })
    }
}

#[cfg(test)]
#[path = "plugin_test.rs"]
mod test;

#[derive(Debug)]
pub struct DojoPlugin {}

impl MacroPlugin for DojoPlugin {
    fn generate_code(&self, db: &dyn SyntaxGroup, item_ast: ast::Item) -> PluginResult {
        match item_ast {
            ast::Item::Module(module_ast) => handle_mod(db, module_ast),
            // Remove other items.
            _ => PluginResult { remove_original_item: true, ..PluginResult::default() },
        }
    }
}

impl AsDynMacroPlugin for DojoPlugin {
    fn as_dyn_macro_plugin<'a>(self: Arc<Self>) -> Arc<dyn MacroPlugin + 'a>
    where
        Self: 'a,
    {
        self
    }
}
impl SemanticPlugin for DojoPlugin {}

fn build_result(
    db: &dyn SyntaxGroup,
    name: String,
    rewrite_nodes: Vec<RewriteNode>,
    diagnostics: Vec<PluginDiagnostic>,
) -> PluginResult {
    let mut builder = PatchBuilder::new(db);
    builder.add_modified(RewriteNode::interpolate_patched(
        &formatdoc!(
            "
                #[contract]
                mod {name} {{
                    $body$
                }}
                ",
        ),
        HashMap::from([(
            "body".to_string(),
            RewriteNode::Modified(ModifiedNode { children: rewrite_nodes }),
        )]),
    ));

    PluginResult {
        code: Some(PluginGeneratedFile {
            name: "contract".into(),
            content: builder.code,
            aux_data: DynGeneratedFileAuxData::new(DynDiagnosticMapper::new(DiagnosticRemapper {
                patches: builder.patches,
            })),
        }),
        diagnostics,
        remove_original_item: true,
    }
}

fn handle_mod(db: &dyn SyntaxGroup, module_ast: ast::ItemModule) -> PluginResult {
    let name = module_ast.name(db).text(db);
    let body = match module_ast.body(db) {
        MaybeModuleBody::Some(body) => body,
        MaybeModuleBody::None(empty_body) => {
            return PluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    message: "Modules without body are not supported.".to_string(),
                    stable_ptr: empty_body.stable_ptr().untyped(),
                }],
                remove_original_item: false,
            };
        }
    };

    if module_ast.has_attr(db, COMPONENT_ATTR) {
        let component = Component::from_module_body(db, body);
        return build_result(db, name.to_string(), component.rewrite_nodes, component.diagnostics);
    }

    if module_ast.has_attr(db, SYSTEM_ATTR) {
        let system = System::from_module_body(db, body);
        return build_result(db, name.to_string(), system.rewrite_nodes, system.diagnostics);
    }

    PluginResult {
            code: None,
            diagnostics: vec![PluginDiagnostic {
                message: "Unsupported module type. Only modules annotated with `component` or `system` supported.".to_string(),
                stable_ptr: module_ast.stable_ptr().untyped(),
            }],
            remove_original_item: true,
        }
}

use std::sync::Arc;

use cairo_lang_defs::plugin::{GeneratedFileAuxData, MacroPlugin, PluginResult};
use cairo_lang_diagnostics::DiagnosticEntry;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::patcher::Patches;
use cairo_lang_semantic::plugin::{
    AsDynGeneratedFileAuxData, AsDynMacroPlugin, PluginAuxData, PluginMappedDiagnostic,
    SemanticPlugin,
};
use cairo_lang_semantic::SemanticDiagnostic;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, Terminal};
use smol_str::SmolStr;

use crate::component::handle_component_struct;
use crate::system::System;

const SYSTEM_ATTR: &str = "system";

#[derive(Debug, PartialEq, Eq)]
pub struct SystemAuxData {
    pub name: SmolStr,
    pub dependencies: Vec<SmolStr>,
}

/// Dojo related auxiliary data of the Dojo plugin.
#[derive(Debug, PartialEq, Eq)]
pub struct DojoAuxData {
    /// Patches of code that need translation in case they have diagnostics.
    pub patches: Patches,

    /// A list of components that were processed by the plugin.
    pub components: Vec<smol_str::SmolStr>,
    /// A list of systems that were processed by the plugin and their component dependencies.
    pub systems: Vec<SystemAuxData>,
}
impl GeneratedFileAuxData for DojoAuxData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn eq(&self, other: &dyn GeneratedFileAuxData) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<Self>() { self == other } else { false }
    }
}
impl AsDynGeneratedFileAuxData for DojoAuxData {
    fn as_dyn_macro_token(&self) -> &(dyn GeneratedFileAuxData + 'static) {
        self
    }
}
impl PluginAuxData for DojoAuxData {
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

#[derive(Debug, Default)]
pub struct DojoPlugin {}

impl DojoPlugin {
    pub fn new() -> Self {
        Self {}
    }

    fn handle_mod(&self, db: &dyn SyntaxGroup, module_ast: ast::ItemModule) -> PluginResult {
        if module_ast.has_attr(db, SYSTEM_ATTR) {
            return System::from_module(db, module_ast);
        }

        PluginResult::default()
    }
}

impl MacroPlugin for DojoPlugin {
    fn generate_code(&self, db: &dyn SyntaxGroup, item_ast: ast::Item) -> PluginResult {
        match item_ast {
            ast::Item::Module(module_ast) => self.handle_mod(db, module_ast),
            ast::Item::Struct(struct_ast) => {
                for attr in struct_ast.attributes(db).elements(db) {
                    if attr.attr(db).text(db) == "component" {
                        return handle_component_struct(db, struct_ast);
                    }
                }

                PluginResult::default()
            }
            _ => PluginResult::default(),
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

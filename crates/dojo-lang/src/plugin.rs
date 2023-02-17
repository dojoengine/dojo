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
use cairo_lang_syntax::node::ast::MaybeModuleBody;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, Terminal};
use dojo_project::WorldConfig;

use crate::component::Component;
use crate::system::System;

const COMPONENT_ATTR: &str = "component";
const SYSTEM_ATTR: &str = "system";

/// Dojo related auxiliary data of the Dojo plugin.
#[derive(Debug, PartialEq, Eq)]
pub struct DojoAuxData {
    /// Patches of code that need translation in case they have diagnostics.
    pub patches: Patches,

    /// A list of components that were processed by the plugin.
    pub components: Vec<smol_str::SmolStr>,
    /// A list of systems that were processed by the plugin.
    pub systems: Vec<smol_str::SmolStr>,
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

#[derive(Debug)]
pub struct DojoPlugin {
    pub world_config: WorldConfig,
}

impl MacroPlugin for DojoPlugin {
    fn generate_code(&self, db: &dyn SyntaxGroup, item_ast: ast::Item) -> PluginResult {
        match item_ast {
            ast::Item::Module(module_ast) => handle_mod(db, self.world_config, module_ast),
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

fn handle_mod(
    db: &dyn SyntaxGroup,
    world_config: WorldConfig,
    module_ast: ast::ItemModule,
) -> PluginResult {
    let name = module_ast.name(db).text(db);
    let body = match module_ast.body(db) {
        MaybeModuleBody::Some(body) => body,
        MaybeModuleBody::None(_empty_body) => {
            return PluginResult::default();
        }
    };

    if module_ast.has_attr(db, COMPONENT_ATTR) {
        return Component::from_module_body(db, name, body).result(db);
    }

    if module_ast.has_attr(db, SYSTEM_ATTR) {
        return System::from_module_body(db, world_config, name, body).result(db);
    }

    PluginResult::default()
}

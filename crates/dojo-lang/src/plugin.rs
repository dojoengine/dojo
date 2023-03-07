use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cairo_lang_defs::plugin::{GeneratedFileAuxData, MacroPlugin, PluginDiagnostic, PluginResult};
use cairo_lang_diagnostics::DiagnosticEntry;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::patcher::{Patches, RewriteNode};
use cairo_lang_semantic::plugin::{
    AsDynGeneratedFileAuxData, AsDynMacroPlugin, PluginAuxData, PluginMappedDiagnostic,
    SemanticPlugin,
};
use cairo_lang_semantic::SemanticDiagnostic;
use cairo_lang_syntax::node::ast::MaybeModuleBody;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use dojo_project::WorldConfig;
use smol_str::SmolStr;
use starknet::core::crypto::pedersen_hash;
use starknet::core::types::FieldElement;

use crate::component::{
    handle_component_impl, handle_component_struct, handle_generated_component,
};
use crate::system::System;

const COMPONENT_ATTR: &str = "generated_component";
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

#[derive(Debug, Default)]
pub struct DojoPlugin {
    pub world_config: WorldConfig,
    pub impls: Arc<Mutex<HashMap<SmolStr, Vec<RewriteNode>>>>,
}

impl DojoPlugin {
    pub fn new(world_config: WorldConfig) -> Self {
        Self { world_config, impls: Arc::new(Mutex::new(HashMap::new())) }
    }

    fn handle_mod(
        &self,
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
            let guard = self.impls.lock().unwrap();
            let impls = guard.get(&name).map_or_else(Vec::new, |vec| vec.clone());
            return handle_generated_component(db, module_ast, impls);
        }

        if module_ast.has_attr(db, SYSTEM_ATTR) {
            return System::from_module_body(db, world_config, name, body).result(db);
        }

        PluginResult::default()
    }
}

impl MacroPlugin for DojoPlugin {
    fn generate_code(&self, db: &dyn SyntaxGroup, item_ast: ast::Item) -> PluginResult {
        match item_ast {
            ast::Item::Module(module_ast) => self.handle_mod(db, self.world_config, module_ast),
            ast::Item::Struct(struct_ast) => {
                for attr in struct_ast.attributes(db).elements(db) {
                    if attr.attr(db).text(db) == "derive" {
                        if let ast::OptionAttributeArgs::AttributeArgs(args) = attr.args(db) {
                            for arg in args.arg_list(db).elements(db) {
                                if let ast::Expr::Path(expr) = arg {
                                    if let [ast::PathSegment::Simple(segment)] =
                                        &expr.elements(db)[..]
                                    {
                                        let derived = segment.ident(db).text(db);
                                        if matches!(derived.as_str(), "Component") {
                                            return handle_component_struct(db, struct_ast);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                PluginResult::default()
            }
            ast::Item::Impl(impl_ast) => {
                if let ast::PathSegment::WithGenericArgs(element) =
                    &impl_ast.trait_path(db).elements(db)[0]
                {
                    if element.ident(db).text(db) != "ComponentTrait" {
                        return PluginResult::default();
                    }

                    let generics = element.generic_args(db).generic_args(db).elements(db);
                    if generics.len() != 1 {
                        return PluginResult {
                            diagnostics: vec![PluginDiagnostic {
                                message: "Must have a single generic parameter.".to_string(),
                                stable_ptr: impl_ast.as_syntax_node().stable_ptr(),
                            }],
                            ..Default::default()
                        };
                    }

                    if let ast::Expr::Path(path) = &generics[0] {
                        if let [ast::PathSegment::Simple(segment)] = &path.elements(db)[..] {
                            let name = segment.ident(db).text(db);
                            if let ast::MaybeImplBody::Some(body) = impl_ast.body(db) {
                                let mut guard = self.impls.lock().unwrap();
                                let rewrite_nodes = handle_component_impl(db, body);
                                guard.entry(name.clone()).or_insert(vec![]);
                                guard
                                    .entry(name)
                                    .and_modify(|vec| vec.extend(rewrite_nodes.to_vec()));
                            }

                            return PluginResult {
                                remove_original_item: true,
                                ..PluginResult::default()
                            };
                        }
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

pub fn get_contract_address(
    module_name: &str,
    class_hash: FieldElement,
    world_address: FieldElement,
) -> FieldElement {
    let mut module_name_32_u8: [u8; 32] = [0; 32];
    module_name_32_u8[32 - module_name.len()..].copy_from_slice(module_name.as_bytes());

    let salt = pedersen_hash(
        &FieldElement::ZERO,
        &FieldElement::from_bytes_be(&module_name_32_u8).unwrap(),
    );
    starknet::core::utils::get_contract_address(salt, class_hash, &[], world_address)
}

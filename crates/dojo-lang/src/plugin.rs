use std::sync::Arc;

use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, GeneratedFileAuxData, MacroPlugin, PluginDiagnostic,
    PluginGeneratedFile, PluginResult,
};
use cairo_lang_diagnostics::DiagnosticEntry;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::patcher::{PatchBuilder, Patches};
use cairo_lang_semantic::plugin::{
    AsDynGeneratedFileAuxData, AsDynMacroPlugin, DynPluginAuxData, PluginAuxData,
    PluginMappedDiagnostic, SemanticPlugin,
};
use cairo_lang_semantic::SemanticDiagnostic;
use cairo_lang_starknet::plugin::StarkNetPlugin;
use cairo_lang_syntax::attribute::structured::{
    AttributeArg, AttributeArgVariant, AttributeStructurize,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, Terminal};
use dojo_types::component::Member;
use dojo_types::system::Dependency;
use scarb::compiler::plugin::builtin::BuiltinSemanticCairoPlugin;
use scarb::core::{PackageId, PackageName, SourceId};
use semver::Version;
use smol_str::SmolStr;
use url::Url;

use crate::component::handle_component_struct;
use crate::system::System;

const SYSTEM_ATTR: &str = "system";

#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub name: String,
    pub members: Vec<Member>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SystemAuxData {
    pub name: SmolStr,
    pub dependencies: Vec<Dependency>,
}

/// Dojo related auxiliary data of the Dojo plugin.
#[derive(Debug, Default, PartialEq)]
pub struct DojoAuxData {
    /// Patches of code that need translation in case they have diagnostics.
    pub patches: Patches,

    /// A list of components that were processed by the plugin.
    pub components: Vec<Component>,
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
pub struct DojoPlugin;

impl DojoPlugin {
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
                let mut aux_data = DojoAuxData::default();
                let mut rewrite_nodes = vec![];
                let mut diagnostics = vec![];

                // Iterate over all the derive attributes of the struct
                for attr in struct_ast.attributes(db).query_attr(db, "derive") {
                    let attr = attr.structurize(db);

                    // Check if the derive attribute has arguments
                    if attr.args.is_empty() {
                        diagnostics.push(PluginDiagnostic {
                            stable_ptr: attr.args_stable_ptr.untyped(),
                            message: "Expected args.".into(),
                        });
                        continue;
                    }

                    // Iterate over all the arguments of the derive attribute
                    for arg in attr.args {
                        // Check if the argument is a path then set it to arg
                        let AttributeArg{
                            variant: AttributeArgVariant::Unnamed {
                                value: ast::Expr::Path(path),
                                ..
                            },
                            ..
                        } = arg else {
                            diagnostics.push(PluginDiagnostic {
                                stable_ptr: arg.arg_stable_ptr.untyped(),
                                message: "Expected path.".into(),
                            });
                            continue;
                        };

                        // Check if the path has a single segment
                        let [ast::PathSegment::Simple(segment)] = &path.elements(db)[..] else {
                            continue;
                        };

                        // Get the text of the segment and check if it is "Component"
                        let derived = segment.ident(db).text(db);

                        match derived.as_str() {
                            "Component" => {
                                rewrite_nodes.push(handle_component_struct(
                                    db,
                                    &mut aux_data,
                                    struct_ast.clone(),
                                ));
                            }
                            _ => continue,
                        }
                    }
                }

                if rewrite_nodes.is_empty() {
                    return PluginResult { diagnostics, ..PluginResult::default() };
                }

                let name = struct_ast.name(db).text(db);
                let mut builder = PatchBuilder::new(db);
                for node in rewrite_nodes {
                    builder.add_modified(node);
                }

                PluginResult {
                    code: Some(PluginGeneratedFile {
                        name,
                        content: builder.code,
                        aux_data: DynGeneratedFileAuxData::new(DynPluginAuxData::new(aux_data)),
                    }),
                    diagnostics: vec![],
                    remove_original_item: true,
                }
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

pub struct CairoPluginRepository(scarb::compiler::plugin::CairoPluginRepository);

impl CairoPluginRepository {
    pub fn new() -> Self {
        let mut repo = scarb::compiler::plugin::CairoPluginRepository::empty();
        let url = Url::parse("https://github.com/dojoengine/dojo").unwrap();
        let dojo_package_id = PackageId::new(
            PackageName::new("dojo_plugin"),
            Version::parse("0.1.0").unwrap(),
            SourceId::for_git(&url, &scarb::core::GitReference::DefaultBranch).unwrap(),
        );
        repo.add(Box::new(BuiltinSemanticCairoPlugin::<DojoPlugin>::new(dojo_package_id))).unwrap();
        let starknet_package_id = PackageId::new(
            PackageName::STARKNET,
            Version::parse("2.0.1").unwrap(),
            SourceId::for_std(),
        );
        repo.add(Box::new(BuiltinSemanticCairoPlugin::<StarkNetPlugin>::new(starknet_package_id)))
            .unwrap();
        Self(repo)
    }
}

impl Default for CairoPluginRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl From<CairoPluginRepository> for scarb::compiler::plugin::CairoPluginRepository {
    fn from(val: CairoPluginRepository) -> Self {
        val.0
    }
}

use std::sync::Arc;

use anyhow::Result;
use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, GeneratedFileAuxData, InlineMacroExprPlugin, MacroPlugin,
    PluginDiagnostic, PluginGeneratedFile, PluginResult,
};
use cairo_lang_syntax::attribute::structured::{
    AttributeArg, AttributeArgVariant, AttributeStructurize,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, Terminal};
use dojo_types::system::Dependency;
use dojo_world::manifest::Member;
use scarb::compiler::plugin::builtin::BuiltinStarkNetPlugin;
use scarb::compiler::plugin::{CairoPlugin, CairoPluginInstance};
use scarb::core::{PackageId, PackageName, SourceId};
use semver::Version;
use smol_str::SmolStr;
use url::Url;

use crate::inline_macros::emit::EmitMacro;
use crate::inline_macros::get::GetMacro;
use crate::inline_macros::set::SetMacro;
use crate::introspect::handle_introspect_struct;
use crate::model::handle_model_struct;
use crate::print::derive_print;
use crate::system::System;

const SYSTEM_ATTR: &str = "system";

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
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
    /// A list of models that were processed by the plugin.
    pub models: Vec<Model>,
    /// A list of systems that were processed by the plugin and their model dependencies.
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

impl CairoPlugin for DojoPlugin {
    fn id(&self) -> PackageId {
        let url = Url::parse("https://github.com/dojoengine/dojo").unwrap();
        PackageId::new(
            PackageName::new("dojo_plugin"),
            Version::parse("0.2.1").unwrap(),
            SourceId::for_git(&url, &scarb::core::GitReference::DefaultBranch).unwrap(),
        )
    }

    fn instantiate(&self) -> Result<Box<dyn CairoPluginInstance>> {
        Ok(Box::new(DojoPluginInstance))
    }
}

struct DojoPluginInstance;
impl CairoPluginInstance for DojoPluginInstance {
    fn macro_plugins(&self) -> Vec<Arc<dyn MacroPlugin>> {
        vec![Arc::new(DojoPlugin)]
    }

    fn inline_macro_plugins(&self) -> Vec<(String, Arc<dyn InlineMacroExprPlugin>)> {
        vec![
            (GetMacro::NAME.into(), Arc::new(GetMacro)),
            (SetMacro::NAME.into(), Arc::new(SetMacro)),
            (EmitMacro::NAME.into(), Arc::new(EmitMacro)),
        ]
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
                        let AttributeArg {
                            variant:
                                AttributeArgVariant::Unnamed { value: ast::Expr::Path(path), .. },
                            ..
                        } = arg
                        else {
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

                        // Get the text of the segment and check if it is "Model"
                        let derived = segment.ident(db).text(db);

                        match derived.as_str() {
                            "Model" => {
                                let (model_rewrite_nodes, model_diagnostics) =
                                    handle_model_struct(db, &mut aux_data, struct_ast.clone());
                                rewrite_nodes.push(model_rewrite_nodes);
                                diagnostics.extend(model_diagnostics);
                            }
                            "Print" => {
                                rewrite_nodes.push(derive_print(db, struct_ast.clone()));
                            }
                            "Introspect" => {
                                rewrite_nodes
                                    .push(handle_introspect_struct(db, struct_ast.clone()));
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
                        aux_data: Some(DynGeneratedFileAuxData::new(aux_data)),
                        diagnostics_mappings: builder.diagnostics_mappings,
                    }),
                    diagnostics,
                    remove_original_item: false,
                }
            }
            _ => PluginResult::default(),
        }
    }
}

pub struct CairoPluginRepository(scarb::compiler::plugin::CairoPluginRepository);

impl CairoPluginRepository {
    pub fn new() -> Self {
        let mut repo = scarb::compiler::plugin::CairoPluginRepository::empty();
        repo.add(Box::new(DojoPlugin)).unwrap();
        repo.add(Box::new(BuiltinStarkNetPlugin)).unwrap();
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

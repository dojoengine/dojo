use std::cmp::Ordering;

use anyhow::Result;
use cairo_lang_defs::patcher::PatchBuilder;
use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, GeneratedFileAuxData, MacroPlugin, MacroPluginMetadata,
    PluginDiagnostic, PluginGeneratedFile, PluginResult,
};
use cairo_lang_diagnostics::Severity;
use cairo_lang_semantic::plugin::PluginSuite;
use cairo_lang_starknet::plugin::aux_data::StarkNetEventAuxData;
use cairo_lang_syntax::attribute::structured::{
    AttributeArg, AttributeArgVariant, AttributeStructurize,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::ids::SyntaxStablePtrId;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode, TypedStablePtr};
use dojo_types::system::Dependency;
use dojo_world::manifest::Member;
use scarb::compiler::plugin::builtin::BuiltinStarkNetPlugin;
use scarb::compiler::plugin::{CairoPlugin, CairoPluginInstance};
use scarb::core::{PackageId, PackageName, SourceId};
use semver::Version;
use smol_str::SmolStr;
use url::Url;

use crate::contract::DojoContract;
use crate::event::handle_event_struct;
use crate::inline_macros::delete::DeleteMacro;
use crate::inline_macros::emit::EmitMacro;
use crate::inline_macros::get::GetMacro;
use crate::inline_macros::set::SetMacro;
use crate::interface::DojoInterface;
use crate::introspect::{handle_introspect_enum, handle_introspect_struct};
use crate::model::handle_model_struct;
use crate::print::{handle_print_enum, handle_print_struct};

pub const DOJO_CONTRACT_ATTR: &str = "dojo::contract";
pub const DOJO_INTERFACE_ATTR: &str = "dojo::interface";
pub const DOJO_MODEL_ATTR: &str = "dojo::model";
pub const DOJO_EVENT_ATTR: &str = "dojo::event";

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
    /// A list of events that were processed by the plugin.
    pub events: Vec<StarkNetEventAuxData>,
}

impl GeneratedFileAuxData for DojoAuxData {
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

/// Dojo related auxiliary data of the Dojo plugin.
#[derive(Debug, Default, PartialEq)]
pub struct ComputedValuesAuxData {
    // Name of entrypoint to get computed value
    pub entrypoint: SmolStr,
    // Model to bind to
    pub model: Option<String>,
}

impl GeneratedFileAuxData for ComputedValuesAuxData {
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

#[cfg(test)]
#[path = "plugin_test.rs"]
mod test;

pub const PACKAGE_NAME: &str = "dojo_plugin";

#[derive(Debug, Default)]
pub struct BuiltinDojoPlugin;

impl BuiltinDojoPlugin {
    fn handle_mod(&self, db: &dyn SyntaxGroup, module_ast: ast::ItemModule) -> PluginResult {
        if module_ast.has_attr(db, DOJO_CONTRACT_ATTR) {
            return DojoContract::from_module(db, module_ast);
        }

        PluginResult::default()
    }

    fn handle_trait(&self, db: &dyn SyntaxGroup, trait_ast: ast::ItemTrait) -> PluginResult {
        if trait_ast.has_attr(db, DOJO_INTERFACE_ATTR) {
            return DojoInterface::from_trait(db, trait_ast);
        }

        PluginResult::default()
    }

    fn result_with_diagnostic(
        &self,
        stable_ptr: SyntaxStablePtrId,
        message: String,
    ) -> PluginResult {
        PluginResult {
            code: None,
            // All diagnostics are for now error. Severity may be moved as argument
            // if warnings are required in this file.
            diagnostics: vec![PluginDiagnostic { stable_ptr, message, severity: Severity::Error }],
            remove_original_item: false,
        }
    }

    fn handle_fn(&self, db: &dyn SyntaxGroup, fn_ast: ast::FunctionWithBody) -> PluginResult {
        let attrs = fn_ast.attributes(db).query_attr(db, "computed");
        if attrs.is_empty() {
            return PluginResult::default();
        }
        if attrs.len() != 1 {
            return self.result_with_diagnostic(
                attrs[0].attr(db).stable_ptr().untyped(),
                format!("Expected one computed macro per function, got {:?}.", attrs.len()),
            );
        }
        let attr = attrs[0].clone().structurize(db);
        let args = attr.args;
        if args.len() > 1 {
            return self.result_with_diagnostic(
                attr.args_stable_ptr.untyped(),
                "Expected one arg for computed macro.\nUsage: #[computed(Position)]".into(),
            );
        }
        let fn_decl = fn_ast.declaration(db);
        let fn_name = fn_decl.name(db).text(db);
        let params = fn_decl.signature(db).parameters(db);
        let param_els = params.elements(db);
        let mut model = None;
        if args.len() == 1 {
            let model_name = args[0].text(db);
            model = Some(model_name.clone());
            let model_type_node = param_els[1].type_clause(db).ty(db);
            if let ast::Expr::Path(model_type_path) = model_type_node {
                let model_type = model_type_path
                    .elements(db)
                    .iter()
                    .last()
                    .unwrap()
                    .as_syntax_node()
                    .get_text(db);
                if model_type != model_name {
                    return self.result_with_diagnostic(
                        model_type_path.stable_ptr().untyped(),
                        "Computed functions second parameter should be the model.".into(),
                    );
                }
            } else {
                return self.result_with_diagnostic(
                    params.stable_ptr().untyped(),
                    format!(
                        "Computed function parameter node of unsupported type {:?}.",
                        model_type_node.as_syntax_node().get_text(db)
                    ),
                );
            }
            if param_els.len() != 2 {
                return self.result_with_diagnostic(
                    params.stable_ptr().untyped(),
                    "Computed function should take 2 parameters, contract state and model.".into(),
                );
            }
        }

        PluginResult {
            code: Some(PluginGeneratedFile {
                name: fn_name.clone(),
                content: "".into(),
                aux_data: Some(DynGeneratedFileAuxData::new(ComputedValuesAuxData {
                    model,
                    entrypoint: fn_name,
                })),
                code_mappings: vec![],
            }),
            diagnostics: vec![],
            remove_original_item: false,
        }
    }
}

impl CairoPlugin for BuiltinDojoPlugin {
    fn id(&self) -> PackageId {
        let url = Url::parse("https://github.com/dojoengine/dojo").unwrap();
        let version = "0.4.0";
        // TODO: update this once pushed.
        let rev = "1e651b5d4d3b79b14a7d8aa29a92062fcb9e6659";

        let source_id =
            SourceId::for_git(&url, &scarb::core::GitReference::Tag(format!("v{version}").into()))
                .unwrap()
                .with_precise(rev.to_string())
                .unwrap();

        PackageId::new(PackageName::new(PACKAGE_NAME), Version::parse(version).unwrap(), source_id)
    }

    fn instantiate(&self) -> Result<Box<dyn CairoPluginInstance>> {
        Ok(Box::new(BuiltinDojoPluginInstance))
    }
}

struct BuiltinDojoPluginInstance;
impl CairoPluginInstance for BuiltinDojoPluginInstance {
    fn plugin_suite(&self) -> PluginSuite {
        dojo_plugin_suite()
    }
}

pub fn dojo_plugin_suite() -> PluginSuite {
    let mut suite = PluginSuite::default();

    suite
        .add_plugin::<BuiltinDojoPlugin>()
        .add_inline_macro_plugin::<DeleteMacro>()
        .add_inline_macro_plugin::<GetMacro>()
        .add_inline_macro_plugin::<SetMacro>()
        .add_inline_macro_plugin::<EmitMacro>();

    suite
}

impl MacroPlugin for BuiltinDojoPlugin {
    // New metadata field: <https://github.com/starkware-libs/cairo/blob/60340c801125b25baaaddce64dd89c6c1524b59d/crates/cairo-lang-defs/src/plugin.rs#L81>
    // Not used for now, but it contains a key-value BTreeSet. TBD what we can do with this.
    fn generate_code(
        &self,
        db: &dyn SyntaxGroup,
        item_ast: ast::ModuleItem,
        _metadata: &MacroPluginMetadata<'_>,
    ) -> PluginResult {
        match item_ast {
            ast::ModuleItem::Module(module_ast) => self.handle_mod(db, module_ast),
            ast::ModuleItem::Trait(trait_ast) => self.handle_trait(db, trait_ast),
            ast::ModuleItem::Enum(enum_ast) => {
                let aux_data = DojoAuxData::default();
                let mut rewrite_nodes = vec![];
                let mut diagnostics = vec![];

                // Iterate over all the derive attributes of the struct
                for attr in enum_ast.attributes(db).query_attr(db, "derive") {
                    let attr = attr.structurize(db);

                    // Check if the derive attribute has arguments
                    if attr.args.is_empty() {
                        diagnostics.push(PluginDiagnostic {
                            stable_ptr: attr.args_stable_ptr.untyped(),
                            message: "Expected args.".into(),
                            severity: Severity::Error,
                        });
                        continue;
                    }

                    // Iterate over all the arguments of the derive attribute
                    for arg in attr.args {
                        // Check if the argument is a path then set it to arg
                        let AttributeArg {
                            variant:
                                AttributeArgVariant::Unnamed(ast::Expr::Path(path)),
                            ..
                        } = arg
                        else {
                            diagnostics.push(PluginDiagnostic {
                                stable_ptr: attr.args_stable_ptr.untyped(),
                                message: "Expected path.".into(),
                                severity: Severity::Error,
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
                            "Introspect" => {
                                rewrite_nodes.push(handle_introspect_enum(
                                    db,
                                    &mut diagnostics,
                                    enum_ast.clone(),
                                ));
                            }
                            "Print" => rewrite_nodes.push(handle_print_enum(db, enum_ast.clone())),
                            _ => continue,
                        }
                    }
                }

                if rewrite_nodes.is_empty() {
                    return PluginResult { diagnostics, ..PluginResult::default() };
                }

                let name = enum_ast.name(db).text(db);
                let mut builder = PatchBuilder::new(db, &enum_ast);
                for node in rewrite_nodes {
                    builder.add_modified(node);
                }

                let (code, code_mappings) = builder.build();

                PluginResult {
                    code: Some(PluginGeneratedFile {
                        name,
                        content: code,
                        aux_data: Some(DynGeneratedFileAuxData::new(aux_data)),
                        code_mappings: code_mappings,
                    }),
                    diagnostics,
                    remove_original_item: false,
                }
            }
            ast::ModuleItem::Struct(struct_ast) => {
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
                            severity: Severity::Error,
                        });
                        continue;
                    }

                    // Iterate over all the arguments of the derive attribute
                    for arg in attr.args {
                        // Check if the argument is a path then set it to arg
                        let AttributeArg {
                            variant:
                                AttributeArgVariant::Unnamed(ast::Expr::Path(path)),
                            ..
                        } = arg
                        else {
                            diagnostics.push(PluginDiagnostic {
                                stable_ptr: attr.args_stable_ptr.untyped(),
                                message: "Expected path.".into(),
                                severity: Severity::Error,
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
                            "Print" => {
                                rewrite_nodes.push(handle_print_struct(db, struct_ast.clone()));
                            }
                            "Introspect" => {
                                rewrite_nodes
                                    .push(handle_introspect_struct(db, struct_ast.clone()));
                            }
                            _ => continue,
                        }
                    }
                }

                let attributes = struct_ast.attributes(db).query_attr(db, DOJO_EVENT_ATTR);

                match attributes.len().cmp(&1) {
                    Ordering::Equal => {
                        let (event_rewrite_nodes, event_diagnostics) =
                            handle_event_struct(db, &mut aux_data, struct_ast.clone());
                        rewrite_nodes.push(event_rewrite_nodes);
                        diagnostics.extend(event_diagnostics);
                    }
                    Ordering::Greater => {
                        diagnostics.push(PluginDiagnostic {
                            message: "A Dojo event must have zero or one dojo::event attribute."
                                .into(),
                            stable_ptr: struct_ast.stable_ptr().untyped(),
                            severity: Severity::Error,
                        });
                    }
                    _ => {}
                }

                let attributes = struct_ast.attributes(db).query_attr(db, DOJO_MODEL_ATTR);

                match attributes.len().cmp(&1) {
                    Ordering::Equal => {
                        let (model_rewrite_nodes, model_diagnostics) =
                            handle_model_struct(db, &mut aux_data, struct_ast.clone());
                        rewrite_nodes.push(model_rewrite_nodes);
                        diagnostics.extend(model_diagnostics);
                    }
                    Ordering::Greater => {
                        diagnostics.push(PluginDiagnostic {
                            message: "A Dojo model must have zero or one dojo::model attribute."
                                .into(),
                            stable_ptr: struct_ast.stable_ptr().untyped(),
                            severity: Severity::Error,
                        });
                    }
                    _ => {}
                }

                if rewrite_nodes.is_empty() {
                    return PluginResult { diagnostics, ..PluginResult::default() };
                }

                let name = struct_ast.name(db).text(db);
                let mut builder = PatchBuilder::new(db, &struct_ast);
                for node in rewrite_nodes {
                    builder.add_modified(node);
                }

                let (code, code_mappings) = builder.build();

                PluginResult {
                    code: Some(PluginGeneratedFile {
                        name,
                        content: code,
                        aux_data: Some(DynGeneratedFileAuxData::new(aux_data)),
                        code_mappings: code_mappings,
                    }),
                    diagnostics,
                    remove_original_item: false,
                }
            }
            ast::ModuleItem::FreeFunction(fn_ast) => self.handle_fn(db, fn_ast),
            _ => PluginResult::default(),
        }
    }

    fn declared_attributes(&self) -> Vec<String> {
        vec![
            DOJO_INTERFACE_ATTR.to_string(),
            DOJO_CONTRACT_ATTR.to_string(),
            DOJO_EVENT_ATTR.to_string(),
            "key".to_string(),
            "computed".to_string(),
            DOJO_MODEL_ATTR.to_string(),
        ]
    }
}

pub struct CairoPluginRepository(scarb::compiler::plugin::CairoPluginRepository);

impl Default for CairoPluginRepository {
    fn default() -> Self {
        let mut repo = scarb::compiler::plugin::CairoPluginRepository::empty();
        repo.add(Box::new(BuiltinDojoPlugin)).unwrap();
        repo.add(Box::new(BuiltinStarkNetPlugin)).unwrap();
        Self(repo)
    }
}

impl From<CairoPluginRepository> for scarb::compiler::plugin::CairoPluginRepository {
    fn from(val: CairoPluginRepository) -> Self {
        val.0
    }
}

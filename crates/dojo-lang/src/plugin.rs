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
use cairo_lang_syntax::attribute::structured::{AttributeArgVariant, AttributeStructurize};
use cairo_lang_syntax::node::ast::Attribute;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::ids::SyntaxStablePtrId;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use dojo_types::system::Dependency;
use dojo_world::manifest::Member;
use dojo_world::utils::split_full_world_element_name;
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
use crate::utils::get_package_id;

pub const DOJO_CONTRACT_ATTR: &str = "dojo::contract";
pub const DOJO_INTERFACE_ATTR: &str = "dojo::interface";
pub const DOJO_MODEL_ATTR: &str = "dojo::model";
pub const DOJO_EVENT_ATTR: &str = "dojo::event";

pub const DOJO_INTROSPECT_ATTR: &str = "Introspect";
pub const DOJO_PACKED_ATTR: &str = "IntrospectPacked";

#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub name: String,
    pub namespace: String,
    pub members: Vec<Member>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SystemAuxData {
    pub name: SmolStr,
    pub namespace: String,
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
        if let Some(other) = other.as_any().downcast_ref::<Self>() { self == other } else { false }
    }
}

/// Dojo related auxiliary data of the Dojo plugin.
#[derive(Debug, Default, PartialEq)]
pub struct ComputedValuesAuxData {
    // Name of entrypoint to get computed value
    pub entrypoint: SmolStr,
    // Model to bind to
    pub namespace: Option<String>,
    pub model: Option<String>,
}

impl GeneratedFileAuxData for ComputedValuesAuxData {
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

pub const PACKAGE_NAME: &str = "dojo_plugin";

#[derive(Debug, Default)]
pub struct BuiltinDojoPlugin;

impl BuiltinDojoPlugin {
    fn handle_mod(
        &self,
        db: &dyn SyntaxGroup,
        module_ast: ast::ItemModule,
        package_id: String,
    ) -> PluginResult {
        if module_ast.has_attr(db, DOJO_CONTRACT_ATTR) {
            return DojoContract::from_module(db, &module_ast, package_id);
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

    fn handle_fn(
        &self,
        db: &dyn SyntaxGroup,
        fn_ast: ast::FunctionWithBody,
        package_id: String,
    ) -> PluginResult {
        let attrs = fn_ast.attributes(db).query_attr(db, "computed");
        if attrs.is_empty() {
            return PluginResult::default();
        }
        if attrs.len() != 1 {
            return self.result_with_diagnostic(
                attrs[0].attr(db).stable_ptr().0,
                format!("Expected one computed macro per function, got {:?}.", attrs.len()),
            );
        }
        let attr = attrs[0].clone().structurize(db);
        let args = attr.args;
        if args.len() > 1 {
            return self.result_with_diagnostic(
                attr.args_stable_ptr.0,
                "Expected one arg for computed macro.\nUsage: #[computed(Position)]".into(),
            );
        }
        let fn_decl = fn_ast.declaration(db);
        let fn_name = fn_decl.name(db).text(db);
        let params = fn_decl.signature(db).parameters(db);
        let param_els = params.elements(db);
        let mut model = None;
        let mut namespace = None;
        if args.len() == 1 {
            let model_name = args[0].text(db);
            match split_full_world_element_name(&model_name, &package_id) {
                Ok((ns, n)) => {
                    model = Some(n);
                    namespace = Some(ns);
                }
                Err(e) => {
                    return self.result_with_diagnostic(attr.args_stable_ptr.0, e.to_string());
                }
            };

            let model_type_node = param_els[1].type_clause(db).ty(db);
            if let ast::Expr::Path(model_type_path) = model_type_node {
                let model_type = model_type_path
                    .elements(db)
                    .iter()
                    .last()
                    .unwrap()
                    .as_syntax_node()
                    .get_text(db);
                if model_type != model_name.clone() {
                    return self.result_with_diagnostic(
                        model_type_path.stable_ptr().0,
                        "Computed functions second parameter should be the model.".into(),
                    );
                }
            } else {
                return self.result_with_diagnostic(
                    params.stable_ptr().0,
                    format!(
                        "Computed function parameter node of unsupported type {:?}.",
                        model_type_node.as_syntax_node().get_text(db)
                    ),
                );
            }
            if param_els.len() != 2 {
                return self.result_with_diagnostic(
                    params.stable_ptr().0,
                    "Computed function should take 2 parameters, contract state and model.".into(),
                );
            }
        }

        PluginResult {
            code: Some(PluginGeneratedFile {
                name: fn_name.clone(),
                content: "".into(),
                aux_data: Some(DynGeneratedFileAuxData::new(ComputedValuesAuxData {
                    namespace,
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

fn get_derive_attr_names(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    attrs: Vec<Attribute>,
) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| {
            let args = attr.clone().structurize(db).args;
            if args.is_empty() {
                diagnostics.push(PluginDiagnostic {
                    stable_ptr: attr.stable_ptr().0,
                    message: "Expected args.".into(),
                    severity: Severity::Error,
                });
                None
            } else {
                Some(args.into_iter().filter_map(|a| {
                    if let AttributeArgVariant::Unnamed(ast::Expr::Path(path)) = a.variant {
                        if let [ast::PathSegment::Simple(segment)] = &path.elements(db)[..] {
                            Some(segment.ident(db).text(db).to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }))
            }
        })
        .flatten()
        .collect::<Vec<_>>()
}

fn check_for_derive_attr_conflicts(
    diagnostics: &mut Vec<PluginDiagnostic>,
    diagnostic_item: SyntaxStablePtrId,
    attr_names: &[String],
) {
    if attr_names.contains(&DOJO_INTROSPECT_ATTR.to_string())
        && attr_names.contains(&DOJO_PACKED_ATTR.to_string())
    {
        diagnostics.push(PluginDiagnostic {
            stable_ptr: diagnostic_item,
            message: format!(
                "{} and {} attributes cannot be used at a same time.",
                DOJO_INTROSPECT_ATTR, DOJO_PACKED_ATTR
            ),
            severity: Severity::Error,
        });
    }
}

fn get_additional_derive_attrs_for_model(derive_attr_names: &[String]) -> Vec<String> {
    let mut additional_attrs = vec![];

    // if not already present, add Introspect to derive attributes because it
    // is mandatory for a model
    if !derive_attr_names.contains(&DOJO_INTROSPECT_ATTR.to_string())
        && !derive_attr_names.contains(&DOJO_PACKED_ATTR.to_string())
    {
        additional_attrs.push(DOJO_INTROSPECT_ATTR.to_string());
    }

    additional_attrs
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
        let package_id = match get_package_id(db) {
            Option::Some(x) => x,
            Option::None => {
                return PluginResult {
                    code: Option::None,
                    diagnostics: vec![PluginDiagnostic {
                        stable_ptr: item_ast.stable_ptr().0,
                        message: "Unable to find the package ID. Be sure to have a 'package.name' \
                                  field in your Scarb.toml file."
                            .into(),
                        severity: Severity::Error,
                    }],
                    remove_original_item: false,
                };
            }
        };

        match item_ast {
            ast::ModuleItem::Module(module_ast) => self.handle_mod(db, module_ast, package_id),
            ast::ModuleItem::Trait(trait_ast) => self.handle_trait(db, trait_ast),
            ast::ModuleItem::Enum(enum_ast) => {
                let aux_data = DojoAuxData::default();
                let mut rewrite_nodes = vec![];
                let mut diagnostics = vec![];

                let derive_attr_names = get_derive_attr_names(
                    db,
                    &mut diagnostics,
                    enum_ast.attributes(db).query_attr(db, "derive"),
                );

                check_for_derive_attr_conflicts(
                    &mut diagnostics,
                    enum_ast.name(db).stable_ptr().0,
                    &derive_attr_names,
                );

                // Iterate over all the derive attributes of the struct
                for attr in derive_attr_names {
                    match attr.as_str() {
                        DOJO_INTROSPECT_ATTR => {
                            rewrite_nodes.push(handle_introspect_enum(
                                db,
                                &mut diagnostics,
                                enum_ast.clone(),
                                false,
                            ));
                        }
                        DOJO_PACKED_ATTR => {
                            rewrite_nodes.push(handle_introspect_enum(
                                db,
                                &mut diagnostics,
                                enum_ast.clone(),
                                true,
                            ));
                        }
                        "Print" => rewrite_nodes.push(handle_print_enum(db, enum_ast.clone())),
                        _ => continue,
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
                        code_mappings,
                    }),
                    diagnostics,
                    remove_original_item: false,
                }
            }
            ast::ModuleItem::Struct(struct_ast) => {
                let mut aux_data = DojoAuxData::default();
                let mut rewrite_nodes = vec![];
                let mut diagnostics = vec![];

                let mut addtional_derive_attr_names = vec![];
                let derive_attr_names = get_derive_attr_names(
                    db,
                    &mut diagnostics,
                    struct_ast.attributes(db).query_attr(db, "derive"),
                );

                let model_attrs = struct_ast.attributes(db).query_attr(db, DOJO_MODEL_ATTR);

                check_for_derive_attr_conflicts(
                    &mut diagnostics,
                    struct_ast.name(db).stable_ptr().0,
                    &derive_attr_names,
                );

                if !model_attrs.is_empty() {
                    addtional_derive_attr_names =
                        get_additional_derive_attrs_for_model(&derive_attr_names);
                }

                // Iterate over all the derive attributes of the struct
                for attr in derive_attr_names.iter().chain(addtional_derive_attr_names.iter()) {
                    match attr.as_str() {
                        "Print" => {
                            rewrite_nodes.push(handle_print_struct(db, struct_ast.clone()));
                        }
                        DOJO_INTROSPECT_ATTR => {
                            rewrite_nodes.push(handle_introspect_struct(
                                db,
                                &mut diagnostics,
                                struct_ast.clone(),
                                false,
                            ));
                        }
                        DOJO_PACKED_ATTR => {
                            rewrite_nodes.push(handle_introspect_struct(
                                db,
                                &mut diagnostics,
                                struct_ast.clone(),
                                true,
                            ));
                        }
                        _ => continue,
                    }
                }

                let event_attrs = struct_ast.attributes(db).query_attr(db, DOJO_EVENT_ATTR);

                match event_attrs.len().cmp(&1) {
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
                            stable_ptr: struct_ast.stable_ptr().0,
                            severity: Severity::Error,
                        });
                    }
                    _ => {}
                }

                match model_attrs.len().cmp(&1) {
                    Ordering::Equal => {
                        let (model_rewrite_nodes, model_diagnostics) =
                            handle_model_struct(db, &mut aux_data, struct_ast.clone(), package_id);
                        rewrite_nodes.push(model_rewrite_nodes);
                        diagnostics.extend(model_diagnostics);
                    }
                    Ordering::Greater => {
                        diagnostics.push(PluginDiagnostic {
                            message: "A Dojo model must have zero or one dojo::model attribute."
                                .into(),
                            stable_ptr: struct_ast.stable_ptr().0,
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
                        code_mappings,
                    }),
                    diagnostics,
                    remove_original_item: false,
                }
            }
            ast::ModuleItem::FreeFunction(fn_ast) => self.handle_fn(db, fn_ast, package_id),
            _ => PluginResult::default(),
        }
    }

    fn declared_attributes(&self) -> Vec<String> {
        vec![
            DOJO_INTERFACE_ATTR.to_string(),
            DOJO_CONTRACT_ATTR.to_string(),
            DOJO_EVENT_ATTR.to_string(),
            DOJO_MODEL_ATTR.to_string(),
            "key".to_string(),
            "computed".to_string(),
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

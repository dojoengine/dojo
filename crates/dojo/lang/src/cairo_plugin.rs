//! Dojo plugin for Cairo.

use cairo_lang_defs::plugin::{MacroPlugin, MacroPluginMetadata, PluginDiagnostic, PluginResult};
use cairo_lang_defs::plugin_utils::PluginResultTrait;
use cairo_lang_diagnostics::Severity;
use cairo_lang_semantic::plugin::PluginSuite;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};

use super::attribute_macros::{
    DojoContract, DojoEvent, DojoModel, DOJO_CONTRACT_ATTR, DOJO_EVENT_ATTR, DOJO_LIBRARY_ATTR,
    DOJO_MODEL_ATTR,
};
use super::derive_macros::{
    dojo_derive_all, DOJO_INTROSPECT_DERIVE, DOJO_LEGACY_STORAGE_DERIVE, DOJO_PACKED_DERIVE,
};
use super::inline_macros::{BytearrayHashMacro, SelectorFromTagMacro};
use crate::attribute_macros::DojoLibrary;

// #[cfg(test)]
// #[path = "plugin_test.rs"]
// mod test;
pub const DOJO_PLUGIN_PACKAGE_NAME: &str = "dojo_plugin";

#[derive(Debug, Default)]
pub struct BuiltinDojoPlugin;

pub fn dojo_plugin_suite() -> PluginSuite {
    let mut suite = PluginSuite::default();

    suite
        .add_plugin::<BuiltinDojoPlugin>()
        .add_inline_macro_plugin::<SelectorFromTagMacro>()
        .add_inline_macro_plugin::<BytearrayHashMacro>();

    suite
}

impl MacroPlugin for BuiltinDojoPlugin {
    /// This function is called for every item in whole db. Hence,
    /// the sooner we can return, the better.
    /// As an example, compiling spawn-and-move project, it's almost 14K calls to this
    /// function.
    ///
    /// Currently Dojo mainly supports:
    /// - Contracts: which are built from attribute macros on a module.
    /// - Interfaces: which are built from attribute macros on a trait.
    /// - Models: which are built from attribute macros on a struct.
    /// - Events: which are built from attribute macros on a struct.
    /// - Enums: mostly used for deriving introspect to be used into a model or event.
    fn generate_code(
        &self,
        db: &dyn SyntaxGroup,
        item_ast: ast::ModuleItem,
        metadata: &MacroPluginMetadata<'_>,
    ) -> PluginResult {
        // Metadata gives information from the crates from where `item_ast` was parsed.
        // During the compilation phase, we inject namespace information into the `CfgSet`
        // so that it can be used here.
        // let namespace_config = metadata.cfg_set.into();

        match &item_ast {
            ast::ModuleItem::Module(module_ast) => {
                if module_ast.has_attr(db, DOJO_CONTRACT_ATTR) {
                    DojoContract::from_module(db, module_ast, metadata)
                } else if module_ast.has_attr(db, DOJO_LIBRARY_ATTR) {
                    DojoLibrary::from_module(db, module_ast, metadata)
                } else {
                    PluginResult::default()
                }
            }
            ast::ModuleItem::Enum(enum_ast) => {
                dojo_derive_all(db, enum_ast.attributes(db).query_attr(db, "derive"), &item_ast)
            }
            ast::ModuleItem::Struct(struct_ast) => {
                let n_model_attrs = struct_ast.attributes(db).query_attr(db, DOJO_MODEL_ATTR).len();

                let n_event_attrs = struct_ast.attributes(db).query_attr(db, DOJO_EVENT_ATTR).len();

                if n_model_attrs > 0 && n_event_attrs > 0 {
                    return PluginResult::diagnostic_only(PluginDiagnostic {
                        stable_ptr: struct_ast.stable_ptr().0,
                        message: format!(
                            "The struct {} can only have one of the dojo::model or one \
                             dojo::event attribute.",
                            struct_ast.name(db).text(db)
                        ),
                        severity: Severity::Error,
                    });
                } else if n_model_attrs == 1 {
                    return DojoModel::from_struct(db, struct_ast.clone());
                } else if n_event_attrs == 1 {
                    return DojoEvent::from_struct(db, struct_ast.clone());
                }

                // Not a model or event, but has derives.
                dojo_derive_all(db, struct_ast.attributes(db).query_attr(db, "derive"), &item_ast)
            }
            _ => PluginResult::default(),
        }
    }

    fn declared_attributes(&self) -> Vec<String> {
        vec![
            DOJO_CONTRACT_ATTR.to_string(),
            DOJO_LIBRARY_ATTR.to_string(),
            DOJO_EVENT_ATTR.to_string(),
            DOJO_MODEL_ATTR.to_string(),
            "key".to_string(),
        ]
    }

    fn declared_derives(&self) -> Vec<String> {
        vec![
            DOJO_INTROSPECT_DERIVE.to_string(),
            DOJO_PACKED_DERIVE.to_string(),
            DOJO_LEGACY_STORAGE_DERIVE.to_string(),
        ]
    }
}

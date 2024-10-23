//! A custom implementation of the starknet::Event derivation path.
//!
//! We append the event selector directly within the append_keys_and_data function.
//! Without the need of the enum for all event variants.
//!
//! <https://github.com/starkware-libs/cairo/blob/main/crates/cairo-lang-starknet/src/plugin/derive/event.rs>

use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, PluginDiagnostic, PluginGeneratedFile, PluginResult,
};
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::{
    ast, ast::ArgClauseNamed, ast::Expr, ast::ModuleItem, db::SyntaxGroup, helpers::QueryAttrs,
    Terminal, TypedStablePtr, TypedSyntaxNode,
};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_types::naming;
use convert_case::{Case, Casing};

use crate::aux_data::EventAuxData;
use crate::derive_macros::{
    extract_derive_attr_names, handle_derive_attrs, DOJO_INTROSPECT_DERIVE, DOJO_PACKED_DERIVE,
};

use super::element::{
    parse_members, serialize_keys_and_values, CommonStructParameters,
    StructParameterParser,
};

const EVENT_PATCH: &str = include_str!("./patches/event.patch.cairo");
use super::DOJO_EVENT_ATTR;

pub const PARAMETER_HISTORICAL: &str = "historical";
pub const DEFAULT_HISTORICAL_VALUE: bool = true;

#[derive(Debug)]
struct EventParameters {
    common: CommonStructParameters,
    historical: bool,
}

impl Default for EventParameters {
    fn default() -> EventParameters {
        EventParameters {
            common: CommonStructParameters::default(),
            historical: DEFAULT_HISTORICAL_VALUE,
        }
    }
}

impl StructParameterParser for EventParameters {
    fn process_named_parameters(
        &mut self,
        db: &dyn SyntaxGroup,
        attribute_name: &str,
        arg: ArgClauseNamed,
        diagnostics: &mut Vec<PluginDiagnostic>,
    ) {
        match arg.name(db).text(db).as_str() {
            PARAMETER_HISTORICAL => {
                self.historical = get_historical(attribute_name, arg.value(db), diagnostics);
            }
            _ => {
                self.common
                    .process_named_parameters(db, attribute_name, arg, diagnostics);
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DojoEvent {}

impl DojoEvent {
    /// A handler for Dojo code that modifies an event struct.
    /// Parameters:
    /// * db: The semantic database.
    /// * struct_ast: The AST of the event struct.
    ///
    /// Returns:
    /// * A RewriteNode containing the generated code.
    pub fn from_struct(
        db: &dyn SyntaxGroup,
        struct_ast: ast::ItemStruct,
    ) -> PluginResult {
        let mut diagnostics = vec![];
        let mut parameters = EventParameters::default();

        parameters.load_from_struct(
            db,
            &DOJO_EVENT_ATTR.to_string(),
            struct_ast.clone(),
            &mut diagnostics,
        );

        let event_name = struct_ast
            .name(db)
            .as_syntax_node()
            .get_text(db)
            .trim()
            .to_string();

        for (id, value) in [("name", &event_name)] {
            if !naming::is_name_valid(value) {
                return PluginResult {
                    code: None,
                    diagnostics: vec![PluginDiagnostic {
                        stable_ptr: struct_ast.stable_ptr().0,
                        message: format!(
                            "The event {id} '{value}' can only contain characters (a-z/A-Z), \
                             digits (0-9) and underscore (_)."
                        ),
                        severity: Severity::Error,
                    }],
                    remove_original_item: false,
                };
            }
        }

        let event_version = parameters.common.version.to_string();
        let event_historical = parameters.historical.to_string();

        let members = parse_members(db, &struct_ast.members(db).elements(db), &mut diagnostics);

        let mut serialized_keys: Vec<RewriteNode> = vec![];
        let mut serialized_values: Vec<RewriteNode> = vec![];

        serialize_keys_and_values(&members, &mut serialized_keys, &mut serialized_values);

        if serialized_keys.is_empty() {
            diagnostics.push(PluginDiagnostic {
                message: "Event must define at least one #[key] attribute".into(),
                stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
                severity: Severity::Error,
            });
        }

        if serialized_values.is_empty() {
            diagnostics.push(PluginDiagnostic {
                message: "Event must define at least one member that is not a key".into(),
                stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
                severity: Severity::Error,
            });
        }

        let member_names = members
            .iter()
            .map(|member| RewriteNode::Text(format!("{},\n", member.name.clone())))
            .collect::<Vec<_>>();

        let mut derive_attr_names = extract_derive_attr_names(
            db,
            &mut diagnostics,
            struct_ast.attributes(db).query_attr(db, "derive"),
        );

        // Ensures events always derive Introspect if not already derived,
        // and do not derive IntrospectPacked.
        if derive_attr_names.contains(&DOJO_PACKED_DERIVE.to_string()) {
            diagnostics.push(PluginDiagnostic {
                message: format!(
                    "Event should derive {DOJO_INTROSPECT_DERIVE} instead of {DOJO_PACKED_DERIVE}."
                ),
                stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
                severity: Severity::Error,
            });
        }

        if !derive_attr_names.contains(&DOJO_INTROSPECT_DERIVE.to_string()) {
            derive_attr_names.push(DOJO_INTROSPECT_DERIVE.to_string());
        }

        let (derive_nodes, derive_diagnostics) = handle_derive_attrs(
            db,
            &derive_attr_names,
            &ModuleItem::Struct(struct_ast.clone()),
        );

        diagnostics.extend(derive_diagnostics);

        let node = RewriteNode::interpolate_patched(
            EVENT_PATCH,
            &UnorderedHashMap::from([
                (
                    "contract_name".to_string(),
                    RewriteNode::Text(event_name.to_case(Case::Snake)),
                ),
                (
                    "type_name".to_string(),
                    RewriteNode::Text(event_name.clone()),
                ),
                (
                    "member_names".to_string(),
                    RewriteNode::new_modified(member_names),
                ),
                (
                    "serialized_keys".to_string(),
                    RewriteNode::new_modified(serialized_keys),
                ),
                (
                    "serialized_values".to_string(),
                    RewriteNode::new_modified(serialized_values),
                ),
                (
                    "event_version".to_string(),
                    RewriteNode::Text(event_version),
                ),
                (
                    "event_historical".to_string(),
                    RewriteNode::Text(event_historical),
                ),
            ]),
        );

        let mut builder = PatchBuilder::new(db, &struct_ast);

        for node in derive_nodes {
            builder.add_modified(node);
        }

        builder.add_modified(node);

        let (code, code_mappings) = builder.build();

        crate::debug_expand(
            &format!("EVENT PATCH: {event_name}"),
            &code,
        );

        let aux_data = EventAuxData {
            name: event_name.clone(),
            members,
        };

        PluginResult {
            code: Some(PluginGeneratedFile {
                name: event_name.into(),
                content: code,
                aux_data: Some(DynGeneratedFileAuxData::new(aux_data)),
                code_mappings,
            }),
            diagnostics,
            remove_original_item: false,
        }
    }
}

/// Get the historical boolean parameter from the `Expr` parameter.
fn get_historical(
    attribute_name: &str,
    arg_value: Expr,
    diagnostics: &mut Vec<PluginDiagnostic>,
) -> bool {
    match arg_value {
        Expr::True(_) => true,
        Expr::False(_) => false,
        _ => {
            diagnostics.push(PluginDiagnostic {
                message: format!(
                    "The argument '{PARAMETER_HISTORICAL}' of {attribute_name} must be a boolean",
                ),
                stable_ptr: arg_value.stable_ptr().untyped(),
                severity: Severity::Error,
            });
            DEFAULT_HISTORICAL_VALUE
        }
    }
}

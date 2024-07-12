use std::collections::HashMap;

use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::{ArgClause, Expr, ItemStruct, OptionArgListParenthesized};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedStablePtr, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use convert_case::{Case, Casing};
use dojo_world::contracts::naming;
use dojo_world::manifest::Member;
use dojo_world::metadata::{is_name_valid, NamespaceConfig};

use crate::plugin::{DojoAuxData, Model, DOJO_MODEL_ATTR};

const DEFAULT_MODEL_VERSION: u8 = 1;

const MODEL_VERSION_NAME: &str = "version";
const MODEL_NAMESPACE: &str = "namespace";
const MODEL_NOMAPPING: &str = "nomapping";

struct ModelParameters {
    version: u8,
    namespace: Option<String>,
    nomapping: bool,
}

impl Default for ModelParameters {
    fn default() -> ModelParameters {
        ModelParameters {
            version: DEFAULT_MODEL_VERSION,
            namespace: Option::None,
            nomapping: false,
        }
    }
}

/// Get the model version from the `Expr` parameter.
fn get_model_version(
    db: &dyn SyntaxGroup,
    arg_value: Expr,
    diagnostics: &mut Vec<PluginDiagnostic>,
) -> u8 {
    match arg_value {
        Expr::Literal(ref value) => {
            if let Ok(value) = value.text(db).parse::<u8>() {
                if value <= DEFAULT_MODEL_VERSION {
                    value
                } else {
                    diagnostics.push(PluginDiagnostic {
                        message: format!("dojo::model version {} not supported", value),
                        stable_ptr: arg_value.stable_ptr().untyped(),
                        severity: Severity::Error,
                    });
                    DEFAULT_MODEL_VERSION
                }
            } else {
                diagnostics.push(PluginDiagnostic {
                    message: format!(
                        "The argument '{}' of dojo::model must be an integer",
                        MODEL_VERSION_NAME
                    ),
                    stable_ptr: arg_value.stable_ptr().untyped(),
                    severity: Severity::Error,
                });
                DEFAULT_MODEL_VERSION
            }
        }
        _ => {
            diagnostics.push(PluginDiagnostic {
                message: format!(
                    "The argument '{}' of dojo::model must be an integer",
                    MODEL_VERSION_NAME
                ),
                stable_ptr: arg_value.stable_ptr().untyped(),
                severity: Severity::Error,
            });
            DEFAULT_MODEL_VERSION
        }
    }
}

/// Get the model namespace from the `Expr` parameter.
fn get_model_namespace(
    db: &dyn SyntaxGroup,
    arg_value: Expr,
    diagnostics: &mut Vec<PluginDiagnostic>,
) -> Option<String> {
    match arg_value {
        Expr::ShortString(ss) => Some(ss.string_value(db).unwrap()),
        Expr::String(s) => Some(s.string_value(db).unwrap()),
        _ => {
            diagnostics.push(PluginDiagnostic {
                message: format!(
                    "The argument '{}' of dojo::model must be a string",
                    MODEL_NAMESPACE
                ),
                stable_ptr: arg_value.stable_ptr().untyped(),
                severity: Severity::Error,
            });
            Option::None
        }
    }
}

/// Get parameters of the dojo::model attribute.
///
/// Note: dojo::model attribute has already been checked so there is one and only one attribute.
///
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the model struct.
/// * diagnostics: vector of compiler diagnostics.
///
/// Returns:
/// * A [`ModelParameters`] object containing all the dojo::model parameters with their
/// default values if not set in the code.
fn get_model_parameters(
    db: &dyn SyntaxGroup,
    struct_ast: ItemStruct,
    diagnostics: &mut Vec<PluginDiagnostic>,
) -> ModelParameters {
    let mut parameters = ModelParameters::default();
    let mut processed_args: HashMap<String, bool> = HashMap::new();

    if let OptionArgListParenthesized::ArgListParenthesized(arguments) =
        struct_ast.attributes(db).query_attr(db, DOJO_MODEL_ATTR).first().unwrap().arguments(db)
    {
        arguments.arguments(db).elements(db).iter().for_each(|a| match a.arg_clause(db) {
            ArgClause::Named(x) => {
                let arg_name = x.name(db).text(db).to_string();
                let arg_value = x.value(db);

                if processed_args.contains_key(&arg_name) {
                    diagnostics.push(PluginDiagnostic {
                        message: format!("Too many '{}' attributes for dojo::model", arg_name),
                        stable_ptr: struct_ast.stable_ptr().untyped(),
                        severity: Severity::Error,
                    });
                } else {
                    processed_args.insert(arg_name.clone(), true);

                    match arg_name.as_str() {
                        MODEL_VERSION_NAME => {
                            parameters.version = get_model_version(db, arg_value, diagnostics);
                        }
                        MODEL_NAMESPACE => {
                            parameters.namespace = get_model_namespace(db, arg_value, diagnostics);
                        }
                        MODEL_NOMAPPING => {
                            parameters.nomapping = true;
                        }
                        _ => {
                            diagnostics.push(PluginDiagnostic {
                                message: format!(
                                    "Unexpected argument '{}' for dojo::model",
                                    arg_name
                                ),
                                stable_ptr: x.stable_ptr().untyped(),
                                severity: Severity::Warning,
                            });
                        }
                    }
                }
            }
            ArgClause::Unnamed(x) => {
                diagnostics.push(PluginDiagnostic {
                    message: format!(
                        "Unexpected argument '{}' for dojo::model",
                        x.as_syntax_node().get_text(db)
                    ),
                    stable_ptr: x.stable_ptr().untyped(),
                    severity: Severity::Warning,
                });
            }
            ArgClause::FieldInitShorthand(x) => {
                diagnostics.push(PluginDiagnostic {
                    message: format!(
                        "Unexpected argument '{}' for dojo::model",
                        x.name(db).name(db).text(db).to_string()
                    ),
                    stable_ptr: x.stable_ptr().untyped(),
                    severity: Severity::Warning,
                });
            }
        })
    }

    parameters
}

/// A handler for Dojo code that modifies a model struct.
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the model struct.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_model_struct(
    db: &dyn SyntaxGroup,
    aux_data: &mut DojoAuxData,
    struct_ast: ItemStruct,
    namespace_config: &NamespaceConfig,
) -> (RewriteNode, Vec<PluginDiagnostic>) {
    let mut diagnostics = vec![];

    let parameters = get_model_parameters(db, struct_ast.clone(), &mut diagnostics);

    let model_name = struct_ast.name(db).as_syntax_node().get_text(db).trim().to_string();
    let unmapped_namespace = parameters.namespace.unwrap_or(namespace_config.default.clone());

    let model_namespace = if parameters.nomapping {
        unmapped_namespace
    } else {
        // Maps namespace from the tag to ensure higher precision on matching namespace mappings.
        namespace_config.get_mapping(&naming::get_tag(&unmapped_namespace, &model_name))
    };

    for (id, value) in [("name", &model_name), ("namespace", &model_namespace)] {
        if !is_name_valid(value) {
            return (
                RewriteNode::empty(),
                vec![PluginDiagnostic {
                    stable_ptr: struct_ast.name(db).stable_ptr().0,
                    message: format!(
                        "The {id} '{value}' can only contain characters (a-z/A-Z), digits (0-9) \
                         and underscore (_)."
                    )
                    .to_string(),
                    severity: Severity::Error,
                }],
            );
        }
    }

    let model_tag = naming::get_tag(&model_namespace, &model_name);
    let model_name_hash = naming::compute_bytearray_hash(&model_name);
    let model_namespace_hash = naming::compute_bytearray_hash(&model_namespace);

    let (model_version, model_selector) = match parameters.version {
        0 => (RewriteNode::Text("0".to_string()), RewriteNode::Text(format!("\"{model_name}\""))),
        _ => (
            RewriteNode::Text(DEFAULT_MODEL_VERSION.to_string()),
            RewriteNode::Text(
                naming::compute_selector_from_hashes(model_namespace_hash, model_name_hash)
                    .to_string(),
            ),
        ),
    };

    let elements = struct_ast.members(db).elements(db);
    let members: &Vec<_> = &elements
        .iter()
        .map(|member| Member {
            name: member.name(db).text(db).to_string(),
            ty: member.type_clause(db).ty(db).as_syntax_node().get_text(db).trim().to_string(),
            key: member.has_attr(db, "key"),
        })
        .collect::<_>();

    let keys: Vec<_> = members.iter().filter(|m| m.key).collect::<_>();

    if keys.is_empty() {
        diagnostics.push(PluginDiagnostic {
            message: "Model must define at least one #[key] attribute".into(),
            stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
            severity: Severity::Error,
        });
    }

    if keys.len() == members.len() {
        diagnostics.push(PluginDiagnostic {
            message: "Model must define at least one member that is not a key".into(),
            stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
            severity: Severity::Error,
        });
    }

    for k in &keys {
        if k.ty == "u256" {
            diagnostics.push(PluginDiagnostic {
                message: "Key is only supported for core types that are 1 felt long once \
                          serialized. `u256` is a struct of 2 u128, hence not supported."
                    .into(),
                stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
                severity: Severity::Error,
            });
        }
    }

    let serialize_member = |m: &Member, include_key: bool| {
        if m.key && !include_key {
            return None;
        }

        if m.ty == "felt252" {
            return Some(RewriteNode::Text(format!(
                "core::array::ArrayTrait::append(ref serialized, *self.{});",
                m.name
            )));
        }

        Some(RewriteNode::Text(format!(
            "core::serde::Serde::serialize(self.{}, ref serialized);",
            m.name
        )))
    };

    let serialized_keys: Vec<_> =
        keys.iter().filter_map(|m| serialize_member(m, true)).collect::<_>();

    let serialized_values: Vec<_> =
        members.iter().filter_map(|m| serialize_member(m, false)).collect::<_>();

    aux_data.models.push(Model {
        name: model_name.clone(),
        namespace: model_namespace.clone(),
        members: members.to_vec(),
    });

    (
        RewriteNode::interpolate_patched(
            "
impl $type_name$Model of dojo::model::Model<$type_name$> {
    fn entity(world: dojo::world::IWorldDispatcher, keys: Span<felt252>, layout: \
             dojo::database::introspect::Layout) -> $type_name$ {
        let values = dojo::world::IWorldDispatcherTrait::entity(
            world,
            Self::selector(),
            keys,
            layout
        );

        // TODO: Generate method to deserialize from keys / values directly to avoid
        // serializing to intermediate array.
        let mut serialized = core::array::ArrayTrait::new();
        core::array::serialize_array_helper(keys, ref serialized);
        core::array::serialize_array_helper(values, ref serialized);
        let mut serialized = core::array::ArrayTrait::span(@serialized);

        let entity = core::serde::Serde::<$type_name$>::deserialize(ref serialized);

        if core::option::OptionTrait::<$type_name$>::is_none(@entity) {
            panic!(
                \"Model `$type_name$`: deserialization failed. Ensure the length of the keys tuple \
             is matching the number of #[key] fields in the model struct.\"
            );
        }

        core::option::OptionTrait::<$type_name$>::unwrap(entity)
    }

    #[inline(always)]
    fn name() -> ByteArray {
        \"$type_name$\"
    }

    #[inline(always)]
    fn namespace() -> ByteArray {
        \"$model_namespace$\"
    }

    #[inline(always)]
    fn tag() -> ByteArray {
        \"$model_tag$\"
    }

    #[inline(always)]
    fn version() -> u8 {
        $model_version$
    }

    #[inline(always)]
    fn selector() -> felt252 {
        $model_selector$
    }

    #[inline(always)]
    fn instance_selector(self: @$type_name$) -> felt252 {
        Self::selector()
    }

    #[inline(always)]
    fn name_hash() -> felt252 {
        $model_name_hash$
    }

    #[inline(always)]
    fn namespace_hash() -> felt252 {
        $model_namespace_hash$
    }
    
    #[inline(always)]
    fn keys(self: @$type_name$) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        $serialized_keys$
        core::array::ArrayTrait::span(@serialized)
    }

    #[inline(always)]
    fn values(self: @$type_name$) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        $serialized_values$
        core::array::ArrayTrait::span(@serialized)
    }

    #[inline(always)]
    fn layout() -> dojo::database::introspect::Layout {
        dojo::database::introspect::Introspect::<$type_name$>::layout()
    }

    #[inline(always)]
    fn instance_layout(self: @$type_name$) -> dojo::database::introspect::Layout {
        Self::layout()
    }

    #[inline(always)]
    fn packed_size() -> Option<usize> {
        let layout = Self::layout();

        match layout {
            dojo::database::introspect::Layout::Fixed(layout) => {
                let mut span_layout = layout;
                Option::Some(dojo::packing::calculate_packed_size(ref span_layout))
            },
            dojo::database::introspect::Layout::Struct(_) => Option::None,
            dojo::database::introspect::Layout::Array(_) => Option::None,
            dojo::database::introspect::Layout::Tuple(_) => Option::None,
            dojo::database::introspect::Layout::Enum(_) => Option::None,
            dojo::database::introspect::Layout::ByteArray => Option::None,
        }
    }
}

#[starknet::interface]
trait I$contract_name$<T> {
    fn ensure_abi(self: @T, model: $type_name$);
}

#[starknet::contract]
mod $contract_name$ {
    use super::$type_name$;
    use super::I$contract_name$;

    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DojoModelImpl of dojo::model::IModel<ContractState>{
        fn name(self: @ContractState) -> ByteArray {
           dojo::model::Model::<$type_name$>::name()
        }

        fn namespace(self: @ContractState) -> ByteArray {
           dojo::model::Model::<$type_name$>::namespace()
        }

        fn tag(self: @ContractState) -> ByteArray {
            dojo::model::Model::<$type_name$>::tag()
        }

        fn version(self: @ContractState) -> u8 {
           dojo::model::Model::<$type_name$>::version()
        }

        fn selector(self: @ContractState) -> felt252 {
           dojo::model::Model::<$type_name$>::selector()
        }

        fn name_hash(self: @ContractState) -> felt252 {
            dojo::model::Model::<$type_name$>::name_hash()
        }

        fn namespace_hash(self: @ContractState) -> felt252 {
            dojo::model::Model::<$type_name$>::namespace_hash()
        }

        fn unpacked_size(self: @ContractState) -> Option<usize> {
            dojo::database::introspect::Introspect::<$type_name$>::size()
        }

        fn packed_size(self: @ContractState) -> Option<usize> {
            dojo::model::Model::<$type_name$>::packed_size()
        }

        fn layout(self: @ContractState) -> dojo::database::introspect::Layout {
            dojo::model::Model::<$type_name$>::layout()
        }

        fn schema(self: @ContractState) -> dojo::database::introspect::Ty {
            dojo::database::introspect::Introspect::<$type_name$>::ty()
        }
    }

    #[abi(embed_v0)]
    impl $contract_name$Impl of I$contract_name$<ContractState>{
        fn ensure_abi(self: @ContractState, model: $type_name$) {
        }
    }
}
",
            &UnorderedHashMap::from([
                ("contract_name".to_string(), RewriteNode::Text(model_name.to_case(Case::Snake))),
                ("type_name".to_string(), RewriteNode::Text(model_name)),
                ("namespace".to_string(), RewriteNode::Text("namespace".to_string())),
                ("serialized_keys".to_string(), RewriteNode::new_modified(serialized_keys)),
                ("serialized_values".to_string(), RewriteNode::new_modified(serialized_values)),
                ("model_version".to_string(), model_version),
                ("model_selector".to_string(), model_selector),
                ("model_namespace".to_string(), RewriteNode::Text(model_namespace.clone())),
                ("model_name_hash".to_string(), RewriteNode::Text(model_name_hash.to_string())),
                (
                    "model_namespace_hash".to_string(),
                    RewriteNode::Text(model_namespace_hash.to_string()),
                ),
                ("model_tag".to_string(), RewriteNode::Text(model_tag.clone())),
            ]),
        ),
        diagnostics,
    )
}

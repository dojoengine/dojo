use std::collections::HashMap;

use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::{
    ArgClause, Expr, ItemStruct, Member as MemberAst, OptionArgListParenthesized,
};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedStablePtr, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_world::config::NamespaceConfig;
use dojo_world::contracts::naming;
use dojo_world::manifest::Member;
use starknet::core::utils::get_selector_from_name;

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
/// * A [`ModelParameters`] object containing all the dojo::model parameters with their default
///   values if not set in the code.
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
///
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
        if !NamespaceConfig::is_name_valid(value) {
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

    let mut members: Vec<Member> = vec![];
    let mut members_values: Vec<RewriteNode> = vec![];
    let mut param_keys: Vec<String> = vec![];
    let mut serialized_keys: Vec<RewriteNode> = vec![];
    let mut serialized_param_keys: Vec<RewriteNode> = vec![];
    let mut serialized_values: Vec<RewriteNode> = vec![];
    let mut field_accessors: Vec<RewriteNode> = vec![];
    let mut entity_field_accessors: Vec<RewriteNode> = vec![];
    let elements = struct_ast.members(db).elements(db);

    elements.iter().for_each(|member_ast| {
        let member = Member {
            name: member_ast.name(db).text(db).to_string(),
            ty: member_ast.type_clause(db).ty(db).as_syntax_node().get_text(db).trim().to_string(),
            key: member_ast.has_attr(db, "key"),
        };

        if member.key {
            validate_key_member(&member, db, member_ast, &mut diagnostics);
            serialized_keys.push(serialize_member_ty(&member, true));
            serialized_param_keys.push(serialize_member_ty(&member, false));
            param_keys.push(format!("{}: {}", member.name, member.ty));
        } else {
            serialized_values.push(serialize_member_ty(&member, true));
            members_values
                .push(RewriteNode::Text(format!("pub {}: {},\n", member.name, member.ty)));
        }

        members.push(member);
    });
    let param_keys = param_keys.join(", ");

    members.iter().filter(|m| !m.key).for_each(|member| {
        field_accessors.push(generate_field_accessors(
            model_name.clone(),
            param_keys.clone(),
            serialized_param_keys.clone(),
            member,
        ));
        entity_field_accessors.push(generate_entity_field_accessors(model_name.clone(), member));
    });

    if serialized_keys.is_empty() {
        diagnostics.push(PluginDiagnostic {
            message: "Model must define at least one #[key] attribute".into(),
            stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
            severity: Severity::Error,
        });
    }

    if serialized_values.is_empty() {
        diagnostics.push(PluginDiagnostic {
            message: "Model must define at least one member that is not a key".into(),
            stable_ptr: struct_ast.name(db).stable_ptr().untyped(),
            severity: Severity::Error,
        });
    }

    aux_data.models.push(Model {
        name: model_name.clone(),
        namespace: model_namespace.clone(),
        members,
    });

    (
        RewriteNode::interpolate_patched(
            "
#[derive(Drop, Serde)]
pub struct $type_name$Entity {
    __id: felt252, // private field
    $members_values$
}

#[generate_trait]
pub impl $type_name$EntityStoreImpl of $type_name$EntityStore {
    fn get(world: dojo::world::IWorldDispatcher, entity_id: felt252) -> $type_name$Entity {
        $type_name$ModelEntityImpl::get(world, entity_id)
    }

    fn update(self: @$type_name$Entity, world: dojo::world::IWorldDispatcher) {
        dojo::model::ModelEntity::<$type_name$Entity>::update_entity(self, world);
    }

    fn delete(self: @$type_name$Entity, world: dojo::world::IWorldDispatcher) {
        dojo::model::ModelEntity::<$type_name$Entity>::delete_entity(self, world);
    }

    $entity_field_accessors$
}

#[generate_trait]
pub impl $type_name$StoreImpl of $type_name$Store {
    fn entity_id_from_keys($param_keys$) -> felt252 {
        let mut serialized = core::array::ArrayTrait::new();
        $serialized_param_keys$
        core::poseidon::poseidon_hash_span(serialized.span())
    }

    fn from_values(ref keys: Span<felt252>, ref values: Span<felt252>) -> $type_name$ {
        let mut serialized = core::array::ArrayTrait::new();
        serialized.append_span(keys);
        serialized.append_span(values);
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

    fn get(world: dojo::world::IWorldDispatcher, $param_keys$) -> $type_name$ {
        let mut serialized = core::array::ArrayTrait::new();
        $serialized_param_keys$

        dojo::model::Model::<$type_name$>::get(world, serialized.span())
    }

    fn set(self: @$type_name$, world: dojo::world::IWorldDispatcher) {
        dojo::model::Model::<$type_name$>::set_model(self, world);
    }

    fn delete(self: @$type_name$, world: dojo::world::IWorldDispatcher) {
        dojo::model::Model::<$type_name$>::delete_model(self, world);
    }

    $field_accessors$
}

pub impl $type_name$ModelEntityImpl of dojo::model::ModelEntity<$type_name$Entity> {
    fn id(self: @$type_name$Entity) -> felt252 {
        *self.__id
    }

    fn values(self: @$type_name$Entity) -> Span<felt252> {
        let mut serialized = core::array::ArrayTrait::new();
        $serialized_values$
        core::array::ArrayTrait::span(@serialized)
    }

    fn from_values(entity_id: felt252, ref values: Span<felt252>) -> $type_name$Entity {
        let mut serialized = array![entity_id];
        serialized.append_span(values);
        let mut serialized = core::array::ArrayTrait::span(@serialized);

        let entity_values = core::serde::Serde::<$type_name$Entity>::deserialize(ref serialized);
        if core::option::OptionTrait::<$type_name$Entity>::is_none(@entity_values) {
            panic!(
                \"ModelEntity `$type_name$Entity`: deserialization failed.\"
            );
        }
        core::option::OptionTrait::<$type_name$Entity>::unwrap(entity_values)
    }

    fn get(world: dojo::world::IWorldDispatcher, entity_id: felt252) -> $type_name$Entity {
        let mut values = dojo::world::IWorldDispatcherTrait::entity(
            world,
            dojo::model::Model::<$type_name$>::selector(),
            dojo::model::ModelIndex::Id(entity_id),
            dojo::model::Model::<$type_name$>::layout()
        );
        Self::from_values(entity_id, ref values)
    }

    fn update_entity(self: @$type_name$Entity, world: dojo::world::IWorldDispatcher) {
        dojo::world::IWorldDispatcherTrait::set_entity(
            world,
            dojo::model::Model::<$type_name$>::selector(),
            dojo::model::ModelIndex::Id(self.id()),
            self.values(),
            dojo::model::Model::<$type_name$>::layout()
        );
    }

    fn delete_entity(self: @$type_name$Entity, world: dojo::world::IWorldDispatcher) {
        dojo::world::IWorldDispatcherTrait::delete_entity(
            world,
            dojo::model::Model::<$type_name$>::selector(),
            dojo::model::ModelIndex::Id(self.id()),
            dojo::model::Model::<$type_name$>::layout()
        );
    }

    fn get_member(
        world: dojo::world::IWorldDispatcher,
        entity_id: felt252,
        member_id: felt252,
    ) -> Span<felt252> {
        match dojo::utils::find_model_field_layout(dojo::model::Model::<$type_name$>::layout(), \
             member_id) {
            Option::Some(field_layout) => {
                dojo::world::IWorldDispatcherTrait::entity(
                    world,
                    dojo::model::Model::<$type_name$>::selector(),
                    dojo::model::ModelIndex::MemberId((entity_id, member_id)),
                    field_layout
                )
            },
            Option::None => core::panic_with_felt252('bad member id')
        }
    }

    fn set_member(
        self: @$type_name$Entity,
        world: dojo::world::IWorldDispatcher,
        member_id: felt252,
        values: Span<felt252>,
    ) {
        match dojo::utils::find_model_field_layout(dojo::model::Model::<$type_name$>::layout(), \
             member_id) {
            Option::Some(field_layout) => {
                dojo::world::IWorldDispatcherTrait::set_entity(
                    world,
                    dojo::model::Model::<$type_name$>::selector(),
                    dojo::model::ModelIndex::MemberId((self.id(), member_id)),
                    values,
                    field_layout
                )
            },
            Option::None => core::panic_with_felt252('bad member id')
        }
    }
}

#[cfg(target: \"test\")]
pub impl $type_name$ModelEntityTestImpl of dojo::model::ModelEntityTest<$type_name$Entity> {
    fn update_test(self: @$type_name$Entity, world: dojo::world::IWorldDispatcher) {
        let world_test = dojo::world::IWorldTestDispatcher { contract_address: \
             world.contract_address };

        dojo::world::IWorldTestDispatcherTrait::set_entity_test(
            world_test,
            dojo::model::Model::<$type_name$>::selector(),
            dojo::model::ModelIndex::Id(self.id()),
            self.values(),
            dojo::model::Model::<$type_name$>::layout()
        );
    }

    fn delete_test(self: @$type_name$Entity, world: dojo::world::IWorldDispatcher) {
        let world_test = dojo::world::IWorldTestDispatcher { contract_address: \
             world.contract_address };

        dojo::world::IWorldTestDispatcherTrait::delete_entity_test(
            world_test,
            dojo::model::Model::<$type_name$>::selector(),
            dojo::model::ModelIndex::Id(self.id()),
            dojo::model::Model::<$type_name$>::layout()
        );
    }
}

pub impl $type_name$ModelImpl of dojo::model::Model<$type_name$> {
    fn get(world: dojo::world::IWorldDispatcher, keys: Span<felt252>) -> $type_name$ {
        let mut values = dojo::world::IWorldDispatcherTrait::entity(
            world,
            Self::selector(),
            dojo::model::ModelIndex::Keys(keys),
            Self::layout()
        );
        let mut _keys = keys;

        $type_name$Store::from_values(ref _keys, ref values)
    }

   fn set_model(
        self: @$type_name$,
        world: dojo::world::IWorldDispatcher
    ) {
        dojo::world::IWorldDispatcherTrait::set_entity(
            world,
            Self::selector(),
            dojo::model::ModelIndex::Keys(Self::keys(self)),
            Self::values(self),
            Self::layout()
        );
    }

    fn delete_model(
        self: @$type_name$,
        world: dojo::world::IWorldDispatcher
    ) {
        dojo::world::IWorldDispatcherTrait::delete_entity(
            world,
            Self::selector(),
            dojo::model::ModelIndex::Keys(Self::keys(self)),
            Self::layout()
        );
    }

    fn get_member(
        world: dojo::world::IWorldDispatcher,
        keys: Span<felt252>,
        member_id: felt252
    ) -> Span<felt252> {
        match dojo::utils::find_model_field_layout(Self::layout(), member_id) {
            Option::Some(field_layout) => {
                let entity_id = dojo::utils::entity_id_from_keys(keys);
                dojo::world::IWorldDispatcherTrait::entity(
                    world,
                    Self::selector(),
                    dojo::model::ModelIndex::MemberId((entity_id, member_id)),
                    field_layout
                )
            },
            Option::None => core::panic_with_felt252('bad member id')
        }
    }

    fn set_member(
        self: @$type_name$,
        world: dojo::world::IWorldDispatcher,
        member_id: felt252,
        values: Span<felt252>
    ) {
        match dojo::utils::find_model_field_layout(Self::layout(), member_id) {
            Option::Some(field_layout) => {
                dojo::world::IWorldDispatcherTrait::set_entity(
                    world,
                    Self::selector(),
                    dojo::model::ModelIndex::MemberId((self.entity_id(), member_id)),
                    values,
                    field_layout
                )
            },
            Option::None => core::panic_with_felt252('bad member id')
        }
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
    fn definition() -> dojo::model::ModelDefinition {
        dojo::model::ModelDefinition {
            selector: $model_selector$,
            namespace: \"$model_namespace$\",
            name: \"$type_name$\",
            version: $model_version$,
            ty: dojo::model::introspect::Introspect::<$type_name$>::ty(),
            layout: dojo::model::introspect::Introspect::<$type_name$>::layout(),
            packed_size: Self::packed_size(),
            unpacked_size: Self::unpacked_size()
        }
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
    fn entity_id(self: @$type_name$) -> felt252 {
        core::poseidon::poseidon_hash_span(self.keys())
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
    fn layout() -> dojo::model::Layout {
        dojo::model::introspect::Introspect::<$type_name$>::layout()
    }

    #[inline(always)]
    fn instance_layout(self: @$type_name$) -> dojo::model::Layout {
        Self::layout()
    }

    #[inline(always)]
    fn ty() -> dojo::model::introspect::Ty {
        dojo::model::introspect::Introspect::<$type_name$>::ty()
    }

    #[inline(always)]
    fn packed_size() -> Option<usize> {
        dojo::model::layout::compute_packed_size(Self::layout())
    }

    #[inline(always)]
    fn unpacked_size() -> Option<usize> {
        dojo::model::introspect::Introspect::<$type_name$>::size()
    }
}

#[cfg(target: \"test\")]
pub impl $type_name$ModelTestImpl of dojo::model::ModelTest<$type_name$> {
   fn set_test(
        self: @$type_name$,
        world: dojo::world::IWorldDispatcher
    ) {
        let world_test = dojo::world::IWorldTestDispatcher { contract_address: \
             world.contract_address };

        dojo::world::IWorldTestDispatcherTrait::set_entity_test(
            world_test,
            dojo::model::Model::<$type_name$>::selector(),
            dojo::model::ModelIndex::Keys(dojo::model::Model::<$type_name$>::keys(self)),
            dojo::model::Model::<$type_name$>::values(self),
            dojo::model::Model::<$type_name$>::layout()
        );
    }

    fn delete_test(
        self: @$type_name$,
        world: dojo::world::IWorldDispatcher
    ) {
        let world_test = dojo::world::IWorldTestDispatcher { contract_address: \
             world.contract_address };

        dojo::world::IWorldTestDispatcherTrait::delete_entity_test(
            world_test,
            dojo::model::Model::<$type_name$>::selector(),
            dojo::model::ModelIndex::Keys(dojo::model::Model::<$type_name$>::keys(self)),
            dojo::model::Model::<$type_name$>::layout()
        );
    }
}
",
            &UnorderedHashMap::from([
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
                ("members_values".to_string(), RewriteNode::new_modified(members_values)),
                ("param_keys".to_string(), RewriteNode::Text(param_keys)),
                (
                    "serialized_param_keys".to_string(),
                    RewriteNode::new_modified(serialized_param_keys),
                ),
                ("field_accessors".to_string(), RewriteNode::new_modified(field_accessors)),
                (
                    "entity_field_accessors".to_string(),
                    RewriteNode::new_modified(entity_field_accessors),
                ),
            ]),
        ),
        diagnostics,
    )
}

/// Validates that the key member is valid.
/// # Arguments
///
/// * member: The member to validate.
/// * diagnostics: The diagnostics to push to, if the member is an invalid key.
fn validate_key_member(
    member: &Member,
    db: &dyn SyntaxGroup,
    member_ast: &MemberAst,
    diagnostics: &mut Vec<PluginDiagnostic>,
) {
    if member.ty == "u256" {
        diagnostics.push(PluginDiagnostic {
            message: "Key is only supported for core types that are 1 felt long once serialized. \
                      `u256` is a struct of 2 u128, hence not supported."
                .into(),
            stable_ptr: member_ast.name(db).stable_ptr().untyped(),
            severity: Severity::Error,
        });
    }
}

/// Creates a [`RewriteNode`] for the member type serialization.
///
/// # Arguments
///
/// * member: The member to serialize.
fn serialize_member_ty(member: &Member, with_self: bool) -> RewriteNode {
    match member.ty.as_str() {
        "felt252" => RewriteNode::Text(format!(
            "core::array::ArrayTrait::append(ref serialized, {}{});\n",
            if with_self { "*self." } else { "" },
            member.name
        )),
        _ => RewriteNode::Text(format!(
            "core::serde::Serde::serialize({}{}, ref serialized);\n",
            if with_self { "self." } else { "@" },
            member.name
        )),
    }
}

/// Generates field accessors (`get_[field_name]` and `set_[field_name]`) for every
/// fields of a model.
///
/// # Arguments
///
/// * `model_name` - the model name.
/// * `param_keys` - coma separated model keys with the format `KEY_NAME: KEY_TYPE`.
/// * `serialized_param_keys` - code to serialize model keys in a `serialized` felt252 array.
/// * `member` - information about the field for which to generate accessors.
///
/// # Returns
/// A [`RewriteNode`] containing accessors code.
fn generate_field_accessors(
    model_name: String,
    param_keys: String,
    serialized_param_keys: Vec<RewriteNode>,
    member: &Member,
) -> RewriteNode {
    RewriteNode::interpolate_patched(
        "
    fn get_$field_name$(world: dojo::world::IWorldDispatcher, $param_keys$) -> $field_type$ {
        let mut serialized = core::array::ArrayTrait::new();
        $serialized_param_keys$

        let mut values = dojo::model::Model::<$model_name$>::get_member(
            world,
            serialized.span(),
            $field_selector$
        );

        let field_value = core::serde::Serde::<$field_type$>::deserialize(ref values);

        if core::option::OptionTrait::<$field_type$>::is_none(@field_value) {
            panic!(
                \"Field `$model_name$::$field_name$`: deserialization failed.\"
            );
        }

        core::option::OptionTrait::<$field_type$>::unwrap(field_value)
    }

    fn set_$field_name$(self: @$model_name$, world: dojo::world::IWorldDispatcher, value: \
         $field_type$) {
        let mut serialized = core::array::ArrayTrait::new();
        core::serde::Serde::serialize(@value, ref serialized);

        self.set_member(
            world,
            $field_selector$,
            serialized.span()
        );
    }
            ",
        &UnorderedHashMap::from([
            ("model_name".to_string(), RewriteNode::Text(model_name)),
            (
                "field_selector".to_string(),
                RewriteNode::Text(
                    get_selector_from_name(&member.name).expect("invalid member name").to_string(),
                ),
            ),
            ("field_name".to_string(), RewriteNode::Text(member.name.clone())),
            ("field_type".to_string(), RewriteNode::Text(member.ty.clone())),
            ("param_keys".to_string(), RewriteNode::Text(param_keys)),
            ("serialized_param_keys".to_string(), RewriteNode::new_modified(serialized_param_keys)),
        ]),
    )
}

/// Generates field accessors (`get_[field_name]` and `set_[field_name]`) for every
/// fields of a model entity.
///
/// # Arguments
///
/// * `model_name` - the model name.
/// * `member` - information about the field for which to generate accessors.
///
/// # Returns
/// A [`RewriteNode`] containing accessors code.
fn generate_entity_field_accessors(model_name: String, member: &Member) -> RewriteNode {
    RewriteNode::interpolate_patched(
        "
    fn get_$field_name$(world: dojo::world::IWorldDispatcher, entity_id: felt252) -> $field_type$ \
         {
        let mut values = dojo::model::ModelEntity::<$model_name$Entity>::get_member(
            world,
            entity_id,
            $field_selector$
        );
        let field_value = core::serde::Serde::<$field_type$>::deserialize(ref values);

        if core::option::OptionTrait::<$field_type$>::is_none(@field_value) {
            panic!(
                \"Field `$model_name$::$field_name$`: deserialization failed.\"
            );
        }

        core::option::OptionTrait::<$field_type$>::unwrap(field_value)
    }

    fn set_$field_name$(self: @$model_name$Entity, world: dojo::world::IWorldDispatcher, value: \
         $field_type$) {
        let mut serialized = core::array::ArrayTrait::new();
        core::serde::Serde::serialize(@value, ref serialized);

        self.set_member(
            world,
            $field_selector$,
            serialized.span()
        );
    }
",
        &UnorderedHashMap::from([
            ("model_name".to_string(), RewriteNode::Text(model_name)),
            (
                "field_selector".to_string(),
                RewriteNode::Text(
                    get_selector_from_name(&member.name).expect("invalid member name").to_string(),
                ),
            ),
            ("field_name".to_string(), RewriteNode::Text(member.name.clone())),
            ("field_type".to_string(), RewriteNode::Text(member.ty.clone())),
        ]),
    )
}

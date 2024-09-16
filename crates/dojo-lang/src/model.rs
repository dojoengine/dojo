use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_syntax::node::ast::ItemStruct;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{TypedStablePtr, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use convert_case::{Case, Casing};
use dojo_world::config::NamespaceConfig;
use dojo_world::contracts::naming;
use dojo_world::manifest::Member;
use starknet::core::utils::get_selector_from_name;

use crate::data::{
    compute_namespace, deserialize_keys_and_values, get_parameters, parse_members,
    serialize_keys_and_values, serialize_member_ty, DEFAULT_DATA_VERSION,
};
use crate::plugin::{DojoAuxData, Model, DOJO_MODEL_ATTR};

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

    let parameters =
        get_parameters(db, &DOJO_MODEL_ATTR.to_string(), struct_ast.clone(), &mut diagnostics);

    let model_name = struct_ast.name(db).as_syntax_node().get_text(db).trim().to_string();
    let model_namespace = compute_namespace(&model_name, &parameters, namespace_config);

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
            RewriteNode::Text(DEFAULT_DATA_VERSION.to_string()),
            RewriteNode::Text(
                naming::compute_selector_from_hashes(model_namespace_hash, model_name_hash)
                    .to_string(),
            ),
        ),
    };

    let members = parse_members(db, &struct_ast.members(db).elements(db), &mut diagnostics);

    let mut serialized_keys: Vec<RewriteNode> = vec![];
    let mut serialized_values: Vec<RewriteNode> = vec![];

    serialize_keys_and_values(
        &members,
        "serialized",
        &mut serialized_keys,
        "serialized",
        &mut serialized_values,
    );

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

    let mut deserialized_keys: Vec<RewriteNode> = vec![];
    let mut deserialized_values: Vec<RewriteNode> = vec![];

    deserialize_keys_and_values(
        &members,
        "keys",
        &mut deserialized_keys,
        "values",
        &mut deserialized_values,
    );

    let mut member_key_names: Vec<RewriteNode> = vec![];
    let mut member_value_names: Vec<RewriteNode> = vec![];
    let mut members_values: Vec<RewriteNode> = vec![];
    let mut param_keys: Vec<String> = vec![];
    let mut serialized_param_keys: Vec<RewriteNode> = vec![];

    members.iter().for_each(|member| {
        if member.key {
            param_keys.push(format!("{}: {}", member.name, member.ty));
            serialized_param_keys.push(serialize_member_ty(member, false, "serialized"));
            member_key_names.push(RewriteNode::Text(format!("{},\n", member.name.clone())));
        } else {
            members_values
                .push(RewriteNode::Text(format!("pub {}: {},\n", member.name, member.ty)));
            member_value_names.push(RewriteNode::Text(format!("{},\n", member.name.clone())));
        }
    });
    let param_keys = param_keys.join(", ");

    let mut field_accessors: Vec<RewriteNode> = vec![];
    let mut entity_field_accessors: Vec<RewriteNode> = vec![];

    members.iter().filter(|m| !m.key).for_each(|member| {
        field_accessors.push(generate_field_accessors(
            model_name.clone(),
            param_keys.clone(),
            serialized_param_keys.clone(),
            member,
        ));
        entity_field_accessors.push(generate_entity_field_accessors(model_name.clone(), member));
    });

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

    fn from_values(ref keys: Span<felt252>, ref values: Span<felt252>) -> Option<$type_name$> {
        $deserialized_keys$
        $deserialized_values$

        Option::Some(
            $type_name$ {
                $member_key_names$
                $member_value_names$
            }
        )
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

    fn from_values(entity_id: felt252, ref values: Span<felt252>) -> Option<$type_name$Entity> {
        $deserialized_values$

        Option::Some(
            $type_name$Entity {
                __id: entity_id,
                $member_value_names$
            }
        )
    }

    fn get(world: dojo::world::IWorldDispatcher, entity_id: felt252) -> $type_name$Entity {
        let mut values = dojo::world::IWorldDispatcherTrait::entity(
            world,
            dojo::model::Model::<$type_name$>::selector(),
            dojo::model::ModelIndex::Id(entity_id),
            dojo::model::Model::<$type_name$>::layout()
        );
        match Self::from_values(entity_id, ref values) {
            Option::Some(x) => x,
            Option::None => {
                panic!(\"ModelEntity `$type_name$Entity`: deserialization failed.\")
            }
        }
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

        match $type_name$Store::from_values(ref _keys, ref values) {
            Option::Some(x) => x,
            Option::None => {
                panic!(\"Model `$type_name$`: deserialization failed.\")
            }
        }
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
    fn layout() -> dojo::meta::Layout {
        dojo::meta::introspect::Introspect::<$type_name$>::layout()
    }

    #[inline(always)]
    fn instance_layout(self: @$type_name$) -> dojo::meta::Layout {
        Self::layout()
    }

    #[inline(always)]
    fn packed_size() -> Option<usize> {
        dojo::meta::layout::compute_packed_size(Self::layout())
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

#[starknet::interface]
pub trait I$contract_name$<T> {
    fn ensure_abi(self: @T, model: $type_name$);
}

#[starknet::contract]
pub mod $contract_name$ {
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
            dojo::meta::introspect::Introspect::<$type_name$>::size()
        }

        fn packed_size(self: @ContractState) -> Option<usize> {
            dojo::model::Model::<$type_name$>::packed_size()
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::model::Model::<$type_name$>::layout()
        }

        fn schema(self: @ContractState) -> dojo::meta::introspect::Ty {
            dojo::meta::introspect::Introspect::<$type_name$>::ty()
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
                ("member_key_names".to_string(), RewriteNode::new_modified(member_key_names)),
                ("member_value_names".to_string(), RewriteNode::new_modified(member_value_names)),
                ("serialized_keys".to_string(), RewriteNode::new_modified(serialized_keys)),
                ("serialized_values".to_string(), RewriteNode::new_modified(serialized_values)),
                ("deserialized_keys".to_string(), RewriteNode::new_modified(deserialized_keys)),
                ("deserialized_values".to_string(), RewriteNode::new_modified(deserialized_values)),
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

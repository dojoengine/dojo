use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_diagnostics::Severity;
use cairo_lang_starknet::plugin::consts::EVENT_TRAIT;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, TypedStablePtr, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use convert_case::{Case, Casing};
use dojo_world::config::NamespaceConfig;
use dojo_world::contracts::naming;

use crate::data::{
    compute_namespace, deserialize_keys_and_values, get_parameters, parse_members,
    serialize_keys_and_values,
};
use crate::plugin::{DojoAuxData, Event, DOJO_EVENT_ATTR};

pub fn handle_event_struct(
    db: &dyn SyntaxGroup,
    aux_data: &mut DojoAuxData,
    struct_ast: ast::ItemStruct,
    namespace_config: &NamespaceConfig,
) -> (RewriteNode, Vec<PluginDiagnostic>) {
    let mut diagnostics = vec![];

    let parameters =
        get_parameters(db, &DOJO_EVENT_ATTR.to_string(), struct_ast.clone(), &mut diagnostics);

    let event_name = struct_ast.name(db).as_syntax_node().get_text(db).trim().to_string();
    let event_namespace = compute_namespace(&event_name, &parameters, namespace_config);

    for (id, value) in [("name", &event_name), ("namespace", &event_namespace)] {
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

    let event_tag = naming::get_tag(&event_namespace, &event_name);
    let event_name_hash = naming::compute_bytearray_hash(&event_name);
    let event_namespace_hash = naming::compute_bytearray_hash(&event_namespace);

    let event_version = parameters.version.to_string();
    let event_selector =
        naming::compute_selector_from_hashes(event_namespace_hash, event_name_hash).to_string();

    let members = parse_members(db, &struct_ast.members(db).elements(db), &mut diagnostics);

    let mut serialized_keys: Vec<RewriteNode> = vec![];
    let mut serialized_values: Vec<RewriteNode> = vec![];

    serialize_keys_and_values(
        &members,
        "keys",
        &mut serialized_keys,
        "data",
        &mut serialized_values,
    );

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

    let mut deserialized_keys: Vec<RewriteNode> = vec![];
    let mut deserialized_values: Vec<RewriteNode> = vec![];

    deserialize_keys_and_values(
        &members,
        "keys",
        &mut deserialized_keys,
        "data",
        &mut deserialized_values,
    );

    let member_names = members
        .iter()
        .map(|member| RewriteNode::Text(format!("{},\n", member.name.clone())))
        .collect::<Vec<_>>();

    aux_data.events.push(Event {
        name: event_name.clone(),
        namespace: event_namespace.clone(),
        members,
    });

    (
        RewriteNode::interpolate_patched(
            "
impl $type_name$StrkEventImpl of $strk_event_trait$<$type_name$> {

    fn append_keys_and_data(
        self: @$type_name$, ref keys: Array<felt252>, ref data: Array<felt252>
    ) {
        core::array::ArrayTrait::append(
            ref keys, dojo::event::Event::<$type_name$>::selector()
        );
        $serialized_keys$
        $serialized_values$
    }

    fn deserialize(
        ref keys: Span<felt252>, ref data: Span<felt252>,
    ) -> Option<$type_name$> {
        let _ = keys.pop_front();

        $deserialized_keys$
        $deserialized_values$

        Option::Some(
            $type_name$ {
                $member_names$
            }
        )
    }
}

pub impl $type_name$EventImpl of dojo::event::Event<$type_name$> {

    #[inline(always)]
    fn name() -> ByteArray {
        \"$type_name$\"
    }

    #[inline(always)]
    fn namespace() -> ByteArray {
        \"$event_namespace$\"
    }

    #[inline(always)]
    fn tag() -> ByteArray {
        \"$event_tag$\"
    }

    #[inline(always)]
    fn version() -> u8 {
        $event_version$
    }

    #[inline(always)]
    fn selector() -> felt252 {
        $event_selector$
    }

    #[inline(always)]
    fn instance_selector(self: @$type_name$) -> felt252 {
        Self::selector()
    }

    #[inline(always)]
    fn name_hash() -> felt252 {
        $event_name_hash$
    }

    #[inline(always)]
    fn namespace_hash() -> felt252 {
        $event_namespace_hash$
    }

    #[inline(always)]
    fn layout() -> dojo::meta::Layout {
        dojo::meta::introspect::Introspect::<$type_name$>::layout()
    }

    #[inline(always)]
    fn packed_size() -> Option<usize> {
        dojo::meta::layout::compute_packed_size(Self::layout())
    }

    #[inline(always)]
    fn unpacked_size() -> Option<usize> {
        dojo::meta::introspect::Introspect::<$type_name$>::size()
    }

    #[inline(always)]
    fn schema(self: @$type_name$) -> dojo::meta::introspect::Ty {
        dojo::meta::introspect::Introspect::<$type_name$>::ty()
    }

}

#[starknet::contract]
pub mod $contract_name$ {
    use super::$type_name$;

    #[storage]
    struct Storage {}

    #[abi(embed_v0)]
    impl DojoEventImpl of dojo::event::IEvent<ContractState>{
        fn name(self: @ContractState) -> ByteArray {
           dojo::event::Event::<$type_name$>::name()
        }

        fn namespace(self: @ContractState) -> ByteArray {
           dojo::event::Event::<$type_name$>::namespace()
        }

        fn tag(self: @ContractState) -> ByteArray {
            dojo::event::Event::<$type_name$>::tag()
        }

        fn version(self: @ContractState) -> u8 {
           dojo::event::Event::<$type_name$>::version()
        }

        fn selector(self: @ContractState) -> felt252 {
           dojo::event::Event::<$type_name$>::selector()
        }

        fn name_hash(self: @ContractState) -> felt252 {
            dojo::event::Event::<$type_name$>::name_hash()
        }

        fn namespace_hash(self: @ContractState) -> felt252 {
            dojo::event::Event::<$type_name$>::namespace_hash()
        }

        fn unpacked_size(self: @ContractState) -> Option<usize> {
            dojo::meta::introspect::Introspect::<$type_name$>::size()
        }

        fn packed_size(self: @ContractState) -> Option<usize> {
            dojo::event::Event::<$type_name$>::packed_size()
        }

        fn layout(self: @ContractState) -> dojo::meta::Layout {
            dojo::event::Event::<$type_name$>::layout()
        }

        fn schema(self: @ContractState) -> dojo::meta::introspect::Ty {
            dojo::meta::introspect::Introspect::<$type_name$>::ty()
        }
    }
}
        ",
            &UnorderedHashMap::from([
                ("strk_event_trait".to_string(), RewriteNode::Text(EVENT_TRAIT.to_string())),
                ("contract_name".to_string(), RewriteNode::Text(event_name.to_case(Case::Snake))),
                ("type_name".to_string(), RewriteNode::Text(event_name)),
                ("member_names".to_string(), RewriteNode::new_modified(member_names)),
                ("serialized_keys".to_string(), RewriteNode::new_modified(serialized_keys)),
                ("serialized_values".to_string(), RewriteNode::new_modified(serialized_values)),
                ("deserialized_keys".to_string(), RewriteNode::new_modified(deserialized_keys)),
                ("deserialized_values".to_string(), RewriteNode::new_modified(deserialized_values)),
                ("event_tag".to_string(), RewriteNode::Text(event_tag)),
                ("event_version".to_string(), RewriteNode::Text(event_version)),
                ("event_selector".to_string(), RewriteNode::Text(event_selector)),
                ("event_namespace".to_string(), RewriteNode::Text(event_namespace.clone())),
                ("event_name_hash".to_string(), RewriteNode::Text(event_name_hash.to_string())),
                (
                    "event_namespace_hash".to_string(),
                    RewriteNode::Text(event_namespace_hash.to_string()),
                ),
            ]),
        ),
        diagnostics,
    )
}

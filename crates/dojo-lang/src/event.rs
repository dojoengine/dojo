use cairo_lang_defs::patcher::{ModifiedNode, RewriteNode};
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_starknet::plugin::aux_data::StarkNetEventAuxData;
use cairo_lang_starknet::plugin::consts::{
    EVENT_TRAIT, EVENT_TYPE_NAME, KEY_ATTR, NESTED_ATTR, SERDE_ATTR,
};
use cairo_lang_starknet::plugin::events::EventData;
use cairo_lang_starknet_classes::abi::EventFieldKind;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, Terminal, TypedStablePtr, TypedSyntaxNode};
use indoc::formatdoc;

use crate::plugin::DojoAuxData;

// A custom implementation of the starknet::Event derivation path.
// We append the event selector directly within the append_keys_and_data function.
// Without the need of the enum for all event variants.

// https://github.com/starkware-libs/cairo/blob/main/crates/cairo-lang-starknet/src/plugin/derive/event.rs

pub fn handle_event_struct(
    db: &dyn SyntaxGroup,
    aux_data: &mut DojoAuxData,
    struct_ast: ast::ItemStruct,
) -> (RewriteNode, Vec<PluginDiagnostic>) {
    let mut diagnostics = vec![];

    // TODO(spapini): Support generics.
    let generic_params = struct_ast.generic_params(db);
    match generic_params {
        ast::OptionWrappedGenericParamList::Empty(_) => {}
        _ => {
            diagnostics.push(PluginDiagnostic::error(
                generic_params.stable_ptr().untyped(),
                format!("{EVENT_TYPE_NAME} structs with generic arguments are unsupported"),
            ));
        }
    }

    // Generate append_keys_and_data() code.
    let mut append_members = vec![];
    let mut deserialize_members = vec![];
    let mut ctor = vec![];
    let mut members = vec![];
    for member in struct_ast.members(db).elements(db) {
        let member_name = RewriteNode::new_trimmed(member.name(db).as_syntax_node());
        let member_kind =
            get_field_kind_for_member(db, &mut diagnostics, &member, EventFieldKind::DataSerde);
        members.push((member.name(db).text(db), member_kind));

        let member_for_append = RewriteNode::interpolate_patched(
            "self.$member_name$",
            &[("member_name".to_string(), member_name.clone())].into(),
        );
        let append_member = append_field(member_kind, member_for_append);
        let deserialize_member = deserialize_field(member_kind, member_name.clone());
        append_members.push(append_member);
        deserialize_members.push(deserialize_member);
        ctor.push(RewriteNode::interpolate_patched(
            "$member_name$, ",
            &[("member_name".to_string(), member_name)].into(),
        ));
    }
    let event_data = EventData::Struct { members };
    aux_data.events.push(StarkNetEventAuxData { event_data });

    let append_members = RewriteNode::Modified(ModifiedNode { children: Some(append_members) });
    let deserialize_members =
        RewriteNode::Modified(ModifiedNode { children: Some(deserialize_members) });
    let ctor = RewriteNode::Modified(ModifiedNode { children: Some(ctor) });

    // Add an implementation for `Event<StructName>`.
    let struct_name = RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node());
    (
        // Append the event selector using the struct_name for the selector
        // and then append the members.
        RewriteNode::interpolate_patched(
            &formatdoc!(
                "
            impl $struct_name$IsEvent of {EVENT_TRAIT}<$struct_name$> {{
                fn append_keys_and_data(
                    self: @$struct_name$, ref keys: Array<felt252>, ref data: Array<felt252>
                ) {{
                    core::array::ArrayTrait::append(ref keys, \
                 dojo::model::Model::<$struct_name$>::selector());
                    $append_members$
                }}
                fn deserialize(
                    ref keys: Span<felt252>, ref data: Span<felt252>,
                ) -> Option<$struct_name$> {{$deserialize_members$
                    Option::Some($struct_name$ {{$ctor$}})
                }}
            }}
            "
            ),
            &[
                ("struct_name".to_string(), struct_name),
                ("append_members".to_string(), append_members),
                ("deserialize_members".to_string(), deserialize_members),
                ("ctor".to_string(), ctor),
            ]
            .into(),
        ),
        diagnostics,
    )
}

/// Generates code to emit an event for a field
fn append_field(member_kind: EventFieldKind, field: RewriteNode) -> RewriteNode {
    match member_kind {
        EventFieldKind::Nested | EventFieldKind::Flat => RewriteNode::interpolate_patched(
            &format!(
                "
                {EVENT_TRAIT}::append_keys_and_data(
                    $field$, ref keys, ref data
                );"
            ),
            &[("field".to_string(), field)].into(),
        ),
        EventFieldKind::KeySerde => RewriteNode::interpolate_patched(
            "
                core::serde::Serde::serialize($field$, ref keys);",
            &[("field".to_string(), field)].into(),
        ),
        EventFieldKind::DataSerde => RewriteNode::interpolate_patched(
            "
            core::serde::Serde::serialize($field$, ref data);",
            &[("field".to_string(), field)].into(),
        ),
    }
}

fn deserialize_field(member_kind: EventFieldKind, member_name: RewriteNode) -> RewriteNode {
    RewriteNode::interpolate_patched(
        match member_kind {
            EventFieldKind::Nested | EventFieldKind::Flat => {
                "
                let $member_name$ = starknet::Event::deserialize(
                    ref keys, ref data
                )?;"
            }
            EventFieldKind::KeySerde => {
                "
                let $member_name$ = core::serde::Serde::deserialize(
                    ref keys
                )?;"
            }
            EventFieldKind::DataSerde => {
                "
                let $member_name$ = core::serde::Serde::deserialize(
                    ref data
                )?;"
            }
        },
        &[("member_name".to_string(), member_name)].into(),
    )
}

/// Retrieves the field kind for a given enum variant,
/// indicating how the field should be serialized.
/// See [EventFieldKind].
fn get_field_kind_for_member(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    member: &ast::Member,
    default: EventFieldKind,
) -> EventFieldKind {
    let is_nested = member.has_attr(db, NESTED_ATTR);
    let is_key = member.has_attr(db, KEY_ATTR);
    let is_serde = member.has_attr(db, SERDE_ATTR);

    // Currently, nested fields are unsupported.
    if is_nested {
        diagnostics.push(PluginDiagnostic::error(
            member.stable_ptr().untyped(),
            "Nested event fields are currently unsupported".to_string(),
        ));
    }
    // Currently, serde fields are unsupported.
    if is_serde {
        diagnostics.push(PluginDiagnostic::error(
            member.stable_ptr().untyped(),
            "Serde event fields are currently unsupported".to_string(),
        ));
    }

    if is_key {
        return EventFieldKind::KeySerde;
    }
    default
}

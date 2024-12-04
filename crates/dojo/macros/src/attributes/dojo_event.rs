//! `dojo_event` attribute macro.

use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_macro::{attribute_macro, Diagnostics, ProcMacroResult, TokenStream};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;

use super::constants::DOJO_EVENT_ATTR;
use super::struct_parser::{
    compute_unique_hash, handle_struct_attribute_macro, parse_members, serialize_keys_and_values,
    validate_attributes, validate_namings_diagnostics,
};
use crate::attributes::struct_parser::remove_derives;
use crate::derives::{extract_derive_attr_names, DOJO_INTROSPECT_DERIVE, DOJO_PACKED_DERIVE};
use crate::diagnostic_ext::DiagnosticsExt;

const EVENT_PATCH: &str = include_str!("./patches/event.patch.cairo");

#[attribute_macro("dojo::event")]
pub fn dojo_event(_args: TokenStream, token_stream: TokenStream) -> ProcMacroResult {
    handle_event_attribute_macro(token_stream)
}

// inner function to be called in tests as `dojo_event()` is automatically renamed
// by the `attribute_macro` processing
pub fn handle_event_attribute_macro(token_stream: TokenStream) -> ProcMacroResult {
    handle_struct_attribute_macro(token_stream, from_struct)
}

pub fn from_struct(db: &dyn SyntaxGroup, struct_ast: &ast::ItemStruct) -> ProcMacroResult {
    let mut diagnostics = vec![];

    let event_name = struct_ast.name(db).as_syntax_node().get_text(db).trim().to_string();

    diagnostics.extend(validate_attributes(db, &struct_ast.attributes(db), DOJO_EVENT_ATTR));

    diagnostics.extend(validate_namings_diagnostics(&[("event name", &event_name)]));

    let members = parse_members(db, &struct_ast.members(db).elements(db), &mut diagnostics);

    let mut serialized_keys: Vec<RewriteNode> = vec![];
    let mut serialized_values: Vec<RewriteNode> = vec![];

    serialize_keys_and_values(&members, &mut serialized_keys, &mut serialized_values);

    if serialized_keys.is_empty() {
        diagnostics.push_error("Event must define at least one #[key] attribute".to_string());
    }

    if serialized_values.is_empty() {
        diagnostics
            .push_error("Event must define at least one member that is not a key".to_string());
    }

    let members_values = members
        .iter()
        .filter_map(|m| {
            if m.key {
                None
            } else {
                Some(RewriteNode::Text(format!("pub {}: {},\n", m.name, m.ty)))
            }
        })
        .collect::<Vec<_>>();

    let member_names = members
        .iter()
        .map(|member| RewriteNode::Text(format!("{},\n", member.name.clone())))
        .collect::<Vec<_>>();

    let derive_attr_names = extract_derive_attr_names(
        db,
        &mut diagnostics,
        struct_ast.attributes(db).query_attr(db, "derive"),
    );

    if derive_attr_names.contains(&DOJO_PACKED_DERIVE.to_string()) {
        diagnostics.push_error(format!("Deriving {DOJO_PACKED_DERIVE} on event is not allowed."));
    }

    let has_drop = derive_attr_names.contains(&"Drop".to_string());
    let has_serde = derive_attr_names.contains(&"Serde".to_string());

    if !has_drop || !has_serde {
        diagnostics.push_error("Event must derive from Drop and Serde.".to_string());
    }

    // Ensures events always derive Introspect if not already derived.
    let derive_node = RewriteNode::Text(format!("#[derive({})]", DOJO_INTROSPECT_DERIVE));

    // Must remove the derives from the original struct since they would create duplicates
    // with the derives of other plugins.
    let original_struct = remove_derives(db, struct_ast);

    let unique_hash =
        compute_unique_hash(db, &event_name, false, &struct_ast.members(db).elements(db))
            .to_string();

    let dojo_node = RewriteNode::interpolate_patched(
        EVENT_PATCH,
        &UnorderedHashMap::from([
            ("derive_node".to_string(), derive_node),
            ("original_struct".to_string(), original_struct),
            ("type_name".to_string(), RewriteNode::Text(event_name.clone())),
            ("member_names".to_string(), RewriteNode::new_modified(member_names)),
            ("serialized_keys".to_string(), RewriteNode::new_modified(serialized_keys)),
            ("serialized_values".to_string(), RewriteNode::new_modified(serialized_values)),
            ("unique_hash".to_string(), RewriteNode::Text(unique_hash)),
            ("members_values".to_string(), RewriteNode::new_modified(members_values)),
        ]),
    );

    let mut builder = PatchBuilder::new(db, struct_ast);
    builder.add_modified(dojo_node);

    let (code, _) = builder.build();

    crate::debug_expand(&format!("EVENT PATCH: {event_name}"), &code);

    ProcMacroResult::new(TokenStream::new(code)).with_diagnostics(Diagnostics::new(diagnostics))
}

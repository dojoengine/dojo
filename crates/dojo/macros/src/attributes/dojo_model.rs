//! `dojo_model` attribute macro.

use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_macro::{attribute_macro, Diagnostics, ProcMacroResult, TokenStream};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use starknet::core::utils::get_selector_from_name;

use super::constants::DOJO_MODEL_ATTR;
use super::struct_parser::{
    compute_unique_hash, handle_struct_attribute_macro, parse_members, serialize_member_ty,
    validate_attributes, validate_namings_diagnostics, Member,
};
use crate::attributes::struct_parser::remove_derives;
use crate::derives::{extract_derive_attr_names, DOJO_INTROSPECT_DERIVE, DOJO_PACKED_DERIVE};
use crate::diagnostic_ext::DiagnosticsExt;

const MODEL_CODE_PATCH: &str = include_str!("./patches/model.patch.cairo");
const MODEL_FIELD_CODE_PATCH: &str = include_str!("./patches/model_field_store.patch.cairo");

#[attribute_macro("dojo::model")]
pub fn dojo_model(_args: TokenStream, token_stream: TokenStream) -> ProcMacroResult {
    handle_model_attribute_macro(token_stream)
}

// inner function to be called in tests as `dojo_model()` is automatically renamed
// by the `attribute_macro` processing
pub fn handle_model_attribute_macro(token_stream: TokenStream) -> ProcMacroResult {
    handle_struct_attribute_macro(token_stream, from_struct)
}

pub fn from_struct(db: &dyn SyntaxGroup, struct_ast: &ast::ItemStruct) -> ProcMacroResult {
    let mut diagnostics = vec![];

    let model_type = struct_ast.name(db).as_syntax_node().get_text(db).trim().to_string();

    diagnostics.extend(validate_attributes(db, &struct_ast.attributes(db), DOJO_MODEL_ATTR));
    diagnostics.extend(validate_namings_diagnostics(&[("model name", &model_type)]));

    let mut values: Vec<Member> = vec![];
    let mut keys: Vec<Member> = vec![];
    let mut members_values: Vec<RewriteNode> = vec![];
    let mut key_types: Vec<String> = vec![];
    let mut key_attrs: Vec<String> = vec![];

    let mut serialized_keys: Vec<RewriteNode> = vec![];
    let mut serialized_values: Vec<RewriteNode> = vec![];
    let mut field_accessors: Vec<RewriteNode> = vec![];

    let members = parse_members(db, &struct_ast.members(db).elements(db), &mut diagnostics);

    members.iter().for_each(|member| {
        if member.key {
            keys.push(member.clone());
            key_types.push(member.ty.clone());
            key_attrs.push(format!("*self.{}", member.name.clone()));
            serialized_keys.push(serialize_member_ty(member, true));
        } else {
            values.push(member.clone());
            serialized_values.push(serialize_member_ty(member, true));
            members_values
                .push(RewriteNode::Text(format!("pub {}: {},\n", member.name, member.ty)));
            field_accessors.push(generate_field_accessors(model_type.clone(), member));
        }
    });

    if keys.is_empty() {
        diagnostics.push_error("Model must define at least one #[key] attribute".to_string());
    }

    if values.is_empty() {
        diagnostics
            .push_error("Model must define at least one member that is not a key".to_string());
    }

    if !diagnostics.is_empty() {
        return ProcMacroResult::new(TokenStream::empty())
            .with_diagnostics(Diagnostics::new(diagnostics));
    }

    let (keys_to_tuple, key_type) = if keys.len() > 1 {
        (format!("({})", key_attrs.join(", ")), format!("({})", key_types.join(", ")))
    } else {
        (key_attrs.first().unwrap().to_string(), key_types.first().unwrap().to_string())
    };

    let derive_attr_names = extract_derive_attr_names(
        db,
        &mut diagnostics,
        struct_ast.attributes(db).query_attr(db, "derive"),
    );

    let has_introspect = derive_attr_names.contains(&DOJO_INTROSPECT_DERIVE.to_string());
    let has_introspect_packed = derive_attr_names.contains(&DOJO_PACKED_DERIVE.to_string());
    let has_drop = derive_attr_names.contains(&"Drop".to_string());
    let has_serde = derive_attr_names.contains(&"Serde".to_string());

    if has_introspect && has_introspect_packed {
        diagnostics.push_error(
            "Model cannot derive from both Introspect and IntrospectPacked.".to_string(),
        );
    }

    #[allow(clippy::nonminimal_bool)]
    if !has_drop || !has_serde {
        diagnostics.push_error("Model must derive from Drop and Serde.".to_string());
    }

    let derive_node = if has_introspect_packed {
        RewriteNode::Text(format!("#[derive({})]", DOJO_PACKED_DERIVE))
    } else {
        RewriteNode::Text(format!("#[derive({})]", DOJO_INTROSPECT_DERIVE))
    };

    // Must remove the derives from the original struct since they would create duplicates
    // with the derives of other plugins.
    let original_struct = remove_derives(db, struct_ast);

    // Reuse the same derive attributes for ModelValue (except Introspect/IntrospectPacked).
    let model_value_derive_attr_names = derive_attr_names
        .iter()
        .map(|d| d.as_str())
        .filter(|&d| d != DOJO_INTROSPECT_DERIVE && d != DOJO_PACKED_DERIVE)
        .collect::<Vec<&str>>()
        .join(", ");

    let unique_hash = compute_unique_hash(
        db,
        &model_type,
        has_introspect_packed,
        &struct_ast.members(db).elements(db),
    )
    .to_string();

    let dojo_node = RewriteNode::interpolate_patched(
        MODEL_CODE_PATCH,
        &UnorderedHashMap::from([
            ("derive_node".to_string(), derive_node),
            ("original_struct".to_string(), original_struct),
            ("model_type".to_string(), RewriteNode::Text(model_type.clone())),
            ("serialized_keys".to_string(), RewriteNode::new_modified(serialized_keys)),
            ("serialized_values".to_string(), RewriteNode::new_modified(serialized_values)),
            ("keys_to_tuple".to_string(), RewriteNode::Text(keys_to_tuple)),
            ("key_type".to_string(), RewriteNode::Text(key_type)),
            ("members_values".to_string(), RewriteNode::new_modified(members_values)),
            ("field_accessors".to_string(), RewriteNode::new_modified(field_accessors)),
            (
                "model_value_derive_attr_names".to_string(),
                RewriteNode::Text(model_value_derive_attr_names),
            ),
            ("unique_hash".to_string(), RewriteNode::Text(unique_hash)),
        ]),
    );

    let mut builder = PatchBuilder::new(db, struct_ast);
    builder.add_modified(dojo_node);

    let (code, _) = builder.build();

    crate::debug_expand(&format!("MODEL PATCH: {model_type}"), &code);

    ProcMacroResult::new(TokenStream::new(code)).with_diagnostics(Diagnostics::new(diagnostics))
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
fn generate_field_accessors(model_type: String, member: &Member) -> RewriteNode {
    RewriteNode::interpolate_patched(
        MODEL_FIELD_CODE_PATCH,
        &UnorderedHashMap::from([
            ("model_type".to_string(), RewriteNode::Text(model_type)),
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

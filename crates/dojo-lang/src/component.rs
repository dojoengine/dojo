use std::collections::HashMap;

use ::serde::{Deserialize, Serialize};
use cairo_lang_defs::plugin::{DynGeneratedFileAuxData, PluginGeneratedFile, PluginResult};
use cairo_lang_filesystem::ids::CrateId;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_semantic::plugin::DynPluginAuxData;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use smol_str::SmolStr;

use crate::plugin::DojoAuxData;

#[cfg(test)]
#[path = "component_test.rs"]
mod test;

/// Struct member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentMember {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// Represents a declaration of a component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    pub name: SmolStr,
    pub members: Vec<ComponentMember>,
}

pub fn handle_component_struct(db: &dyn SyntaxGroup, struct_ast: ast::ItemStruct) -> PluginResult {
    let mut body_nodes = vec![];
    let mut trait_nodes = vec![];

    body_nodes.push(RewriteNode::interpolate_patched(
        "
            #[view]
            fn name() -> felt252 {
                '$type_name$'
            }

            #[view]
            fn len() -> usize {
                $len$_usize
            }

            // Serialize an entity.
            #[view]
            fn serialize(mut raw: Span<felt252>) -> $type_name$ {
                serde::Serde::<$type_name$>::deserialize(ref raw).unwrap()
            }

            // Get the state of an entity.
            #[view]
            #[raw_output]
            fn deserialize(value: $type_name$) -> Span<felt252> {
                let mut arr = ArrayTrait::new();
                serde::Serde::<$type_name$>::serialize(ref arr, value);
                arr.span()
            }
        ",
        HashMap::from([
            (
                "type_name".to_string(),
                RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node()),
            ),
            ("members".to_string(), RewriteNode::Copied(struct_ast.members(db).as_syntax_node())),
            (
                "len".to_string(),
                RewriteNode::Text(struct_ast.members(db).elements(db).len().to_string()),
            ),
        ]),
    ));

    let mut serialize = vec![];
    let mut deserialize = vec![];
    struct_ast.members(db).elements(db).iter().for_each(|member| {
        serialize.push(RewriteNode::interpolate_patched(
            "serde::Serde::<$type_clause$>::serialize(ref serialized, input.$key$);",
            HashMap::from([
                ("key".to_string(), RewriteNode::new_trimmed(member.name(db).as_syntax_node())),
                (
                    "type_clause".to_string(),
                    RewriteNode::new_trimmed(member.type_clause(db).ty(db).as_syntax_node()),
                ),
            ]),
        ));

        deserialize.push(RewriteNode::interpolate_patched(
            "$key$: serde::Serde::<$type_clause$>::deserialize(ref serialized)?,",
            HashMap::from([
                ("key".to_string(), RewriteNode::new_trimmed(member.name(db).as_syntax_node())),
                (
                    "type_clause".to_string(),
                    RewriteNode::new_trimmed(member.type_clause(db).ty(db).as_syntax_node()),
                ),
            ]),
        ));
    });

    trait_nodes.push(RewriteNode::interpolate_patched(
        "
            impl $type_name$Serde of serde::Serde::<$type_name$> {
                fn serialize(ref serialized: Array::<felt252>, input: $type_name$) {
                    $serialize$
                }
                fn deserialize(ref serialized: Span::<felt252>) -> Option::<$type_name$> {
                    Option::Some(
                        $type_name$ {
                            $deserialize$
                        }
                    )
                }
            }
        ",
        HashMap::from([
            (
                "type_name".to_string(),
                RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node()),
            ),
            ("serialize".to_string(), RewriteNode::new_modified(serialize)),
            ("deserialize".to_string(), RewriteNode::new_modified(deserialize)),
        ]),
    ));

    let name = struct_ast.name(db).text(db);
    let mut builder = PatchBuilder::new(db);
    builder.add_modified(RewriteNode::interpolate_patched(
        "
            #[derive(Copy, Drop)]
            struct $type_name$ {
                $members$
            }

            #[abi]
            trait I$type_name$ {
                fn name() -> felt252;
                fn len() -> u8;
                fn serialize(raw: Span<felt252>) -> $type_name$;
                fn deserialize(value: $type_name$) -> Span<felt252>;
            }

            $traits$

            #[contract]
            mod $type_name$Component {
                use array::ArrayTrait;
                use option::OptionTrait;
                use dojo::serde::SpanSerde;
                use super::$type_name$;
                $body$
            }
        ",
        HashMap::from([
            (
                "type_name".to_string(),
                RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node()),
            ),
            ("members".to_string(), RewriteNode::Copied(struct_ast.members(db).as_syntax_node())),
            ("traits".to_string(), RewriteNode::new_modified(trait_nodes)),
            ("body".to_string(), RewriteNode::new_modified(body_nodes)),
        ]),
    ));

    PluginResult {
        code: Some(PluginGeneratedFile {
            name: name.clone(),
            content: builder.code,
            aux_data: DynGeneratedFileAuxData::new(DynPluginAuxData::new(DojoAuxData {
                patches: builder.patches,
                components: vec![format!("{}", name).into()],
                systems: vec![],
            })),
        }),
        diagnostics: vec![],
        remove_original_item: true,
    }
}

/// Finds the inline modules annotated as components in the given crate_ids and
/// returns the corresponding Components.
pub fn find_components(db: &dyn SemanticGroup, crate_ids: &[CrateId]) -> Vec<Component> {
    let mut components = vec![];
    for crate_id in crate_ids {
        let modules = db.crate_modules(*crate_id);
        for module_id in modules.iter() {
            let generated_file_infos =
                db.module_generated_file_infos(*module_id).unwrap_or_default();

            for generated_file_info in generated_file_infos.iter().skip(1) {
                let Some(generated_file_info) = generated_file_info else { continue; };
                let Some(mapper) = generated_file_info.aux_data.0.as_any(
                ).downcast_ref::<DynPluginAuxData>() else { continue; };
                let Some(aux_data) = mapper.0.as_any(
                ).downcast_ref::<DojoAuxData>() else { continue; };

                for name in &aux_data.components {
                    let structs = db.module_structs_ids(*module_id);
                    let component_struct = structs.unwrap()[0];
                    let members = db
                        .struct_members(component_struct)
                        .unwrap()
                        .iter()
                        .map(|(name, member)| ComponentMember {
                            name: name.to_string(),
                            ty: member.ty.format(db),
                        })
                        .collect();
                    components.push(Component { name: name.clone(), members });
                }
            }
        }
    }
    components
}

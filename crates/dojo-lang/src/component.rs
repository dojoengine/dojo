use std::collections::HashMap;

use cairo_lang_defs::ids::{ModuleItemId, SubmoduleId};
use cairo_lang_defs::plugin::{DynGeneratedFileAuxData, PluginGeneratedFile, PluginResult};
use cairo_lang_filesystem::ids::CrateId;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_semantic::plugin::DynPluginAuxData;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use dojo_project::WorldConfig;
use smol_str::SmolStr;

use crate::plugin::{get_contract_address, DojoAuxData};

#[cfg(test)]
#[path = "component_test.rs"]
mod test;

/// Represents a declaration of a component.
#[derive(Debug, Clone)]
pub struct ComponentDeclaration {
    /// The id of the module that defines the component.
    pub submodule_id: SubmoduleId,
    pub name: SmolStr,
}

pub fn handle_component_struct(db: &dyn SyntaxGroup, struct_ast: ast::ItemStruct) -> PluginResult {
    let mut body_nodes = vec![];
    let mut trait_nodes = vec![];

    body_nodes.push(RewriteNode::interpolate_patched(
        "
            struct Storage {
                state: LegacyMap::<usize, $type_name$>,
            }

            // Initialize $type_name$.
            #[external]
            fn initialize() {
            }

            // Set the state of an entity.
            #[external]
            fn set(entity_id: usize, value: $type_name$) {
                state::write(entity_id, value);
            }

            // Get the state of an entity.
            #[view]
            fn get(entity_id: usize) -> $type_name$ {
                return state::read(entity_id);
            }
        ",
        HashMap::from([
            (
                "type_name".to_string(),
                RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node()),
            ),
            ("members".to_string(), RewriteNode::Copied(struct_ast.members(db).as_syntax_node())),
        ]),
    ));

    let mut serialize = vec![];
    let mut deserialize = vec![];
    let mut read = vec![];
    let mut write = vec![];
    struct_ast.members(db).elements(db).iter().enumerate().for_each(|(i, member)| {
        let member_type_string = member.type_clause(db).ty(db).as_syntax_node().get_text(db);
        let is_felt = member_type_string.as_str().trim().eq("felt252");

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

        let try_into_unwrap_token = if is_felt { "?," } else { "?.try_into().unwrap()," };
        read.push(RewriteNode::interpolate_patched(
            format!(
                "$key$: starknet::storage_read_syscall(
                    address_domain,
                    starknet::storage_address_from_base_and_offset(base, $offset$_u8)
                ){try_into_unwrap_token}"
            )
            .as_str(),
            HashMap::from([
                ("key".to_string(), RewriteNode::new_trimmed(member.name(db).as_syntax_node())),
                (
                    "type_clause".to_string(),
                    RewriteNode::new_trimmed(member.type_clause(db).ty(db).as_syntax_node()),
                ),
                ("offset".to_string(), RewriteNode::Text(i.to_string())),
            ]),
        ));

        let into_token = if is_felt { "" } else { ".into()" };
        let final_token =
            if i != struct_ast.members(db).elements(db).len() - 1 { "?;" } else { "" };
        write.push(RewriteNode::interpolate_patched(
            format!(
                "starknet::storage_write_syscall(
                    address_domain,
                    starknet::storage_address_from_base_and_offset(base, $offset$_u8),
                    value.$key${into_token}
                ){final_token}"
            )
            .as_str(),
            HashMap::from([
                ("key".to_string(), RewriteNode::new_trimmed(member.name(db).as_syntax_node())),
                (
                    "type_clause".to_string(),
                    RewriteNode::new_trimmed(member.type_clause(db).ty(db).as_syntax_node()),
                ),
                ("offset".to_string(), RewriteNode::Text(i.to_string())),
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

    trait_nodes.push(RewriteNode::interpolate_patched(
        "
            impl StorageAccess$type_name$ of starknet::StorageAccess::<$type_name$> {
                fn read(address_domain: felt252, base: starknet::StorageBaseAddress) -> \
         starknet::SyscallResult::<$type_name$> {
                    Result::Ok(
                        $type_name$ {
                            $read$
                        }
                    )
                }
                fn write(
                    address_domain: felt252, base: starknet::StorageBaseAddress, value: $type_name$
                ) -> starknet::SyscallResult::<()> {
                    $write$
                }
            }
        ",
        HashMap::from([
            (
                "type_name".to_string(),
                RewriteNode::new_trimmed(struct_ast.name(db).as_syntax_node()),
            ),
            ("read".to_string(), RewriteNode::new_modified(read)),
            ("write".to_string(), RewriteNode::new_modified(write)),
        ]),
    ));

    let name = struct_ast.name(db).text(db);
    let mut builder = PatchBuilder::new(db);
    builder.add_modified(RewriteNode::interpolate_patched(
        "
            use option::OptionTrait;
            use starknet::SyscallResult;
            use traits::Into;
            use traits::TryInto;
        
            #[derive(Copy, Drop)]
            struct $type_name$ {
                $members$
            }
            $traits$
            #[abi]
            trait I$type_name$ {
                fn set(entity_id: usize, value: $type_name$);
                fn get(entity_id: usize) -> $type_name$;
            }
            #[contract]
            mod $type_name$Component {
                use super::$type_name$;
                use super::$type_name$Serde;
                use super::StorageAccess$type_name$;
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
                components: vec![format!("{}Component", name).into()],
                systems: vec![],
            })),
        }),
        diagnostics: vec![],
        remove_original_item: true,
    }
}

/// Finds the inline modules annotated as components in the given crate_ids and
/// returns the corresponding ComponentDeclarations.
pub fn find_components(db: &dyn SemanticGroup, crate_ids: &[CrateId]) -> Vec<ComponentDeclaration> {
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
                    if let Ok(Some(ModuleItemId::Submodule(submodule_id))) =
                        db.module_item_by_name(*module_id, name.clone())
                    {
                        components.push(ComponentDeclaration { name: name.clone(), submodule_id });
                    } else {
                        panic!("Component `{name}` was not found.");
                    }
                }
            }
        }
    }
    components
}

pub fn compute_component_id(
    db: &dyn SyntaxGroup,
    path: ast::ExprPath,
    world_config: WorldConfig,
) -> String {
    // Component name to felt
    let component_name_raw = path.as_syntax_node().get_text(db);
    let mut component_name_parts: Vec<&str> = component_name_raw.split("::").collect();
    let component_name = component_name_parts.pop().unwrap();

    format!(
        "{:#x}",
        get_contract_address(
            component_name,
            world_config.initializer_class_hash.unwrap_or_default(),
            world_config.address.unwrap_or_default(),
        )
    )
}

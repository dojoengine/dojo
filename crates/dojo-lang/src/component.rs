use std::collections::HashMap;

use cairo_lang_defs::ids::{ModuleItemId, SubmoduleId};
use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, PluginDiagnostic, PluginGeneratedFile, PluginResult,
};
use cairo_lang_filesystem::ids::CrateId;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_semantic::plugin::DynPluginAuxData;
use cairo_lang_syntax::node::ast::MaybeModuleBody;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use smol_str::SmolStr;

use crate::plugin::DojoAuxData;

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

pub fn handle_generated_component(
    db: &dyn SyntaxGroup,
    module_ast: ast::ItemModule,
    impls: Vec<RewriteNode>,
) -> PluginResult {
    let name = module_ast.name(db).text(db);

    if let MaybeModuleBody::Some(body) = module_ast.body(db) {
        let mut builder = PatchBuilder::new(db);
        builder.add_modified(RewriteNode::interpolate_patched(
            "
                #[contract]
                mod $name$Component {
                    $body$
                    $members$
                }
            ",
            HashMap::from([
                ("name".to_string(), RewriteNode::Text(name.clone().into())),
                ("members".to_string(), RewriteNode::new_modified(impls)),
                (
                    "body".to_string(),
                    RewriteNode::new_modified(
                        body.items(db)
                            .elements(db)
                            .iter()
                            .map(|item| RewriteNode::Copied(item.as_syntax_node()))
                            .collect::<Vec<_>>(),
                    ),
                ),
            ]),
        ));

        return PluginResult {
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
        };
    }

    PluginResult {
        diagnostics: vec![PluginDiagnostic {
            message: "Component must have a body".to_string(),
            stable_ptr: module_ast.as_syntax_node().stable_ptr(),
        }],
        ..PluginResult::default()
    }
}

pub fn handle_component_impl(db: &dyn SyntaxGroup, body: ast::ImplBody) -> Vec<RewriteNode> {
    let mut rewrite_nodes = vec![];
    for item in body.items(db).elements(db) {
        if let ast::Item::FreeFunction(item_function) = &item {
            let declaration = item_function.declaration(db);

            let mut func_declaration = RewriteNode::from_ast(&declaration);
            func_declaration
                .modify_child(db, ast::FunctionDeclaration::INDEX_SIGNATURE)
                .modify_child(db, ast::FunctionSignature::INDEX_PARAMETERS)
                .modify(db)
                .children
                .as_mut()
                .unwrap()
                .splice(0..1, vec![RewriteNode::Text("entity_id: felt".to_string())]);

            rewrite_nodes.push(RewriteNode::interpolate_patched(
                "
                                    #[view]
                                    $func_decl$ {
                                        let self = state::read(entity_id);
                                        $body$
                                    }
                                    ",
                HashMap::from([
                    ("func_decl".to_string(), func_declaration),
                    (
                        "body".to_string(),
                        RewriteNode::new_trimmed(
                            item_function.body(db).statements(db).as_syntax_node(),
                        ),
                    ),
                ]),
            ));
        }
    }

    rewrite_nodes
}

pub fn handle_component_struct(db: &dyn SyntaxGroup, struct_ast: ast::ItemStruct) -> PluginResult {
    let mut rewrite_nodes = vec![];
    let mut trait_nodes = vec![];

    rewrite_nodes.push(RewriteNode::interpolate_patched(
        "
            struct Storage {
                state: LegacyMap::<felt, $type_name$>,
            }

            // Initialize $type_name$.
            #[external]
            fn initialize() {
            }

            // Set the state of an entity.
            #[external]
            fn set(entity_id: felt, value: $type_name$) {
                state::write(entity_id, value);
            }

            // Get the state of an entity.
            #[view]
            fn get(entity_id: felt) -> $type_name$ {
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
        serialize.push(RewriteNode::interpolate_patched(
            "serde::Serde::<felt>::serialize(ref serialized, input.$key$);",
            HashMap::from([(
                "key".to_string(),
                RewriteNode::new_trimmed(member.name(db).as_syntax_node()),
            )]),
        ));

        deserialize.push(RewriteNode::interpolate_patched(
            "$key$: serde::Serde::<felt>::deserialize(ref serialized)?,",
            HashMap::from([(
                "key".to_string(),
                RewriteNode::new_trimmed(member.name(db).as_syntax_node()),
            )]),
        ));

        read.push(RewriteNode::interpolate_patched(
            "$key$: starknet::storage_read_syscall(
                address_domain, starknet::storage_address_from_base_and_offset(base, $offset$_u8)
            )?,",
            HashMap::from([
                ("key".to_string(), RewriteNode::new_trimmed(member.name(db).as_syntax_node())),
                ("offset".to_string(), RewriteNode::Text(i.to_string())),
            ]),
        ));

        let final_token =
            if i != struct_ast.members(db).elements(db).len() - 1 { "?;" } else { "" };
        write.push(RewriteNode::interpolate_patched(
            format!(
                "
                starknet::storage_write_syscall(
                    address_domain, starknet::storage_address_from_base_and_offset(base, \
                 $offset$_u8), value.$key$){final_token}"
            )
            .as_str(),
            HashMap::from([
                ("key".to_string(), RewriteNode::new_trimmed(member.name(db).as_syntax_node())),
                ("offset".to_string(), RewriteNode::Text(i.to_string())),
            ]),
        ));
    });

    trait_nodes.push(RewriteNode::interpolate_patched(
        "
            impl $type_name$Serde of serde::Serde::<$type_name$> {
                fn serialize(ref serialized: Array::<felt>, input: $type_name$) {
                    $serialize$
                }
                fn deserialize(ref serialized: Span::<felt>) -> Option::<$type_name$> {
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
                fn read(address_domain: felt, base: starknet::StorageBaseAddress) -> \
         starknet::SyscallResult::<$type_name$> {
                    Result::Ok(
                        $type_name$ {
                            $read$
                        }
                    )
                }
                fn write(
                    address_domain: felt, base: starknet::StorageBaseAddress, value: $type_name$
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
            #[generated_component]
            mod $type_name$ {
                #[derive(Copy, Drop)]
                struct $type_name$ {
                    $members$
                }
                $traits$
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
            ("body".to_string(), RewriteNode::new_modified(rewrite_nodes)),
        ]),
    ));

    PluginResult {
        code: Some(PluginGeneratedFile {
            name: name.clone(),
            content: builder.code,
            aux_data: DynGeneratedFileAuxData::new(DynPluginAuxData::new(DojoAuxData {
                patches: builder.patches,
                components: vec![],
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

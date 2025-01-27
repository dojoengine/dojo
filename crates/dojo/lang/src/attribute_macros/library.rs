use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, MacroPluginMetadata, PluginDiagnostic, PluginGeneratedFile,
    PluginResult,
};
use cairo_lang_diagnostics::Severity;
use cairo_lang_plugins::plugins::HasItemsInCfgEx;
use cairo_lang_syntax::node::ast::MaybeModuleBody;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_types::naming;

use crate::aux_data::ContractAuxData;

const LIBRARY_PATCH: &str = include_str!("./patches/library.patch.cairo");
const CONSTRUCTOR_FN: &str = "constructor";
const DOJO_INIT_FN: &str = "dojo_init";

#[derive(Debug, Clone, Default)]
pub struct ContractParameters {
    pub namespace: Option<String>,
}

#[derive(Debug)]
pub struct DojoLibrary {
    diagnostics: Vec<PluginDiagnostic>,
    systems: Vec<String>,
}

impl DojoLibrary {
    pub fn from_module(
        db: &dyn SyntaxGroup,
        module_ast: &ast::ItemModule,
        metadata: &MacroPluginMetadata<'_>,
    ) -> PluginResult {
        let name = module_ast.name(db).text(db);

        let mut library = DojoLibrary { diagnostics: vec![], systems: vec![] };

        for (id, value) in [("name", &name.to_string())] {
            if !naming::is_name_valid(value) {
                return PluginResult {
                    code: None,
                    diagnostics: vec![PluginDiagnostic {
                        stable_ptr: module_ast.stable_ptr().0,
                        message: format!(
                            "The contract {id} '{value}' can only contain characters (a-z/A-Z), \
                             digits (0-9) and underscore (_)."
                        ),
                        severity: Severity::Error,
                    }],
                    remove_original_item: false,
                };
            }
        }

        let mut has_event = false;
        let mut has_storage = false;
        let mut has_init = false;
        let mut has_constructor = false;

        if let MaybeModuleBody::Some(body) = module_ast.body(db) {
            let mut body_nodes: Vec<_> = body
                .iter_items_in_cfg(db, metadata.cfg_set)
                .flat_map(|el| {
                    if let ast::ModuleItem::Enum(ref enum_ast) = el {
                        if enum_ast.name(db).text(db).to_string() == "Event" {
                            has_event = true;
                            return library.merge_event(db, enum_ast.clone());
                        }
                    } else if let ast::ModuleItem::Struct(ref struct_ast) = el {
                        if struct_ast.name(db).text(db).to_string() == "Storage" {
                            has_storage = true;
                            return library.merge_storage(db, struct_ast.clone());
                        }
                    } else if let ast::ModuleItem::FreeFunction(ref fn_ast) = el {
                        let fn_decl = fn_ast.declaration(db);
                        let fn_name = fn_decl.name(db).text(db);

                        if fn_name == CONSTRUCTOR_FN {
                            has_constructor = true;
                        }

                        if fn_name == DOJO_INIT_FN {
                            has_init = true;
                        }
                    }

                    vec![RewriteNode::Copied(el.as_syntax_node())]
                })
                .collect();

            if has_constructor {
                return PluginResult {
                    code: None,
                    diagnostics: vec![PluginDiagnostic {
                        stable_ptr: module_ast.stable_ptr().0,
                        message: format!("The library {name} cannot have a constructor"),
                        severity: Severity::Error,
                    }],
                    remove_original_item: false,
                };
            }

            if has_init {
                return PluginResult {
                    code: None,
                    diagnostics: vec![PluginDiagnostic {
                        stable_ptr: module_ast.stable_ptr().0,
                        message: format!("The library {name} cannot have a dojo_init"),
                        severity: Severity::Error,
                    }],
                    remove_original_item: false,
                };
            }

            if !has_event {
                body_nodes.append(&mut library.create_event())
            }

            if !has_storage {
                body_nodes.append(&mut library.create_storage())
            }

            let mut builder = PatchBuilder::new(db, module_ast);
            builder.add_modified(RewriteNode::Mapped {
                node: Box::new(RewriteNode::interpolate_patched(
                    LIBRARY_PATCH,
                    &UnorderedHashMap::from([
                        ("name".to_string(), RewriteNode::Text(name.to_string())),
                        ("body".to_string(), RewriteNode::new_modified(body_nodes)),
                    ]),
                )),
                origin: module_ast.as_syntax_node().span_without_trivia(db),
            });

            let (code, code_mappings) = builder.build();

            crate::debug_expand(&format!("LIBRARY PATCH: {name}"), &code);

            return PluginResult {
                code: Some(PluginGeneratedFile {
                    name: name.clone(),
                    content: code,
                    aux_data: Some(DynGeneratedFileAuxData::new(ContractAuxData {
                        name: name.to_string(),
                        systems: library.systems.clone(),
                    })),
                    code_mappings,
                    diagnostics_note: None,
                }),
                diagnostics: library.diagnostics,
                remove_original_item: true,
            };
        }

        PluginResult::default()
    }

    pub fn merge_event(
        &mut self,
        db: &dyn SyntaxGroup,
        enum_ast: ast::ItemEnum,
    ) -> Vec<RewriteNode> {
        let mut rewrite_nodes = vec![];

        let elements = enum_ast.variants(db).elements(db);

        let variants = elements.iter().map(|e| e.as_syntax_node().get_text(db)).collect::<Vec<_>>();
        let variants = variants.join(",\n");

        rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
            #[event]
            #[derive(Drop, starknet::Event)]
            enum Event {
                WorldProviderEvent: world_provider_cpt::Event,
                $variants$
            }
            ",
            &UnorderedHashMap::from([("variants".to_string(), RewriteNode::Text(variants))]),
        ));
        rewrite_nodes
    }

    pub fn create_event(&mut self) -> Vec<RewriteNode> {
        vec![RewriteNode::Text(
            "
            #[event]
            #[derive(Drop, starknet::Event)]
            enum Event {
                #[flat]
                WorldProviderEvent: world_provider_cpt::Event,
            }
            "
            .to_string(),
        )]
    }

    pub fn merge_storage(
        &mut self,
        db: &dyn SyntaxGroup,
        struct_ast: ast::ItemStruct,
    ) -> Vec<RewriteNode> {
        let mut rewrite_nodes = vec![];

        let elements = struct_ast.members(db).elements(db);

        let members = elements.iter().map(|e| e.as_syntax_node().get_text(db)).collect::<Vec<_>>();
        let members = members.join(",\n");

        rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
            #[storage]
            struct Storage {
                #[substorage(v0)]
                world_provider: world_provider_cpt::Storage,
                $members$
            }
            ",
            &UnorderedHashMap::from([("members".to_string(), RewriteNode::Text(members))]),
        ));
        rewrite_nodes
    }

    pub fn create_storage(&mut self) -> Vec<RewriteNode> {
        vec![RewriteNode::Text(
            "
            #[storage]
            struct Storage {
            #[substorage(v0)]
                world_provider: world_provider_cpt::Storage,
            }
            "
            .to_string(),
        )]
    }
}

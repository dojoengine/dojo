use std::collections::HashMap;

use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, PluginDiagnostic, PluginGeneratedFile, PluginResult,
};
// use cairo_lang_syntax::node::ast::{MaybeModuleBody, Param};
use cairo_lang_syntax::node::ast::MaybeModuleBody;
// use cairo_lang_syntax::node::ast::OptionReturnTypeClause::ReturnTypeClause;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_types::system::Dependency;

use crate::plugin::{DojoAuxData, SystemAuxData};

pub struct DojoContract {
    diagnostics: Vec<PluginDiagnostic>,
    dependencies: HashMap<smol_str::SmolStr, Dependency>,
}

impl DojoContract {
    pub fn from_module(db: &dyn SyntaxGroup, module_ast: ast::ItemModule) -> PluginResult {
        let name = module_ast.name(db).text(db);
        let mut system = DojoContract { diagnostics: vec![], dependencies: HashMap::new() };
        let mut has_event = false;
        let mut has_storage = false;

        if let MaybeModuleBody::Some(body) = module_ast.body(db) {
            let mut body_nodes: Vec<_> = body
                .items(db)
                .elements(db)
                .iter()
                .flat_map(|el| {
                    if let ast::Item::Enum(enum_ast) = el {
                        if enum_ast.name(db).text(db).to_string() == "Event" {
                            has_event = true;
                            return system.merge_event(db, enum_ast.clone());
                        }
                    } else if let ast::Item::Struct(struct_ast) = el {
                        if struct_ast.name(db).text(db).to_string() == "Storage" {
                            has_storage = true;
                            return system.merge_storage(db, struct_ast.clone());
                        }
                    }

                    vec![RewriteNode::Copied(el.as_syntax_node())]
                })
                .collect();

            if !has_event {
                body_nodes.append(&mut system.create_event())
            }

            if !has_storage {
                body_nodes.append(&mut system.create_storage())
            }

            let mut builder = PatchBuilder::new(db);
            builder.add_modified(RewriteNode::interpolate_patched(
                "
                #[starknet::contract]
                mod $name$ {
                    use dojo::world;
                    use dojo::world::IWorldDispatcher;
                    use dojo::world::IWorldDispatcherTrait;
                    use dojo::world::IWorldProvider;
                   
                    component!(path: dojo::components::upgradeable::upgradeable, storage: \
                 upgradeable, event: UpgradeableEvent);

                    #[external(v0)]
                    fn dojo_resource(self: @ContractState) -> felt252 {
                        '$name$'
                    }

                    #[external(v0)]
                    impl WorldProviderImpl of IWorldProvider<ContractState> {
                        fn world(self: @ContractState) -> IWorldDispatcher {
                            self.world_dispatcher.read()
                        }
                    }

                    #[abi(embed_v0)]
                    impl UpgradableImpl = \
                 dojo::components::upgradeable::upgradeable::UpgradableImpl<ContractState>;

                    $body$
                }
                ",
                &UnorderedHashMap::from([
                    ("name".to_string(), RewriteNode::Text(name.to_string())),
                    ("body".to_string(), RewriteNode::new_modified(body_nodes)),
                ]),
            ));

            return PluginResult {
                code: Some(PluginGeneratedFile {
                    name: name.clone(),
                    content: builder.code,
                    aux_data: Some(DynGeneratedFileAuxData::new(DojoAuxData {
                        models: vec![],
                        systems: vec![SystemAuxData {
                            name,
                            dependencies: system.dependencies.values().cloned().collect(),
                        }],
                    })),
                    diagnostics_mappings: builder.diagnostics_mappings,
                }),
                diagnostics: system.diagnostics,
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
                UpgradeableEvent: dojo::components::upgradeable::upgradeable::Event,
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
                UpgradeableEvent: dojo::components::upgradeable::upgradeable::Event,
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
                world_dispatcher: IWorldDispatcher,
                #[substorage(v0)]
                upgradeable: dojo::components::upgradeable::upgradeable::Storage,
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
                world_dispatcher: IWorldDispatcher,
                #[substorage(v0)]
                upgradeable: dojo::components::upgradeable::upgradeable::Storage,
            }
            "
            .to_string(),
        )]
    }
}

// fn is_context(db: &dyn SyntaxGroup, param: &Param) -> bool {
//     param.type_clause(db).ty(db).as_syntax_node().get_text(db) == "Context"
// }

use std::collections::HashMap;

use cairo_lang_defs::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, PluginDiagnostic, PluginGeneratedFile, PluginResult,
};
// use cairo_lang_syntax::node::ast::{MaybeModuleBody, Param};
use cairo_lang_syntax::node::ast::MaybeModuleBody;
use cairo_lang_syntax::node::ast::OptionReturnTypeClause::ReturnTypeClause;
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

        if let MaybeModuleBody::Some(body) = module_ast.body(db) {
            let body_nodes = body
                .items(db)
                .elements(db)
                .iter()
                .flat_map(|el| {
                    if let ast::Item::FreeFunction(fn_ast) = el {
                        if fn_ast.declaration(db).name(db).text(db).to_string() == "execute" {
                            return system.handle_execute(db, fn_ast.clone());
                        }
                    }

                    vec![RewriteNode::Copied(el.as_syntax_node())]
                })
                .collect();

            let mut builder = PatchBuilder::new(db);
            builder.add_modified(RewriteNode::interpolate_patched(
                "
                #[starknet::contract]
                mod $name$ {
                    use dojo::world;
                    use dojo::world::IWorldDispatcher;
                    use dojo::world::IWorldDispatcherTrait;

                    #[storage]
                    struct Storage {
                        world_dispatcher: IWorldDispatcher,
                    }

                    #[external(v0)]
                    fn name(self: @ContractState) -> felt252 {
                        '$name$'
                    }

                    #[external(v0)]
                    impl Upgradeable of dojo::upgradable::IUpgradeable<ContractState> {
                        fn upgrade(ref self: ContractState, new_class_hash: starknet::ClassHash) {
                            let caller = starknet::get_caller_address();
                            assert(
                                self.world_dispatcher.read().contract_address == caller, 'only \
                 World can upgrade'
                            );
                            dojo::upgradable::UpgradeableTrait::upgrade(new_class_hash);
                        }
                    }

                    $body$
                }
                ",
                UnorderedHashMap::from([
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

    pub fn handle_execute(
        &mut self,
        db: &dyn SyntaxGroup,
        function_ast: ast::FunctionWithBody,
    ) -> Vec<RewriteNode> {
        let mut rewrite_nodes = vec![];

        let signature = function_ast.declaration(db).signature(db);

        let parameters = signature.parameters(db);
        let elements = parameters.elements(db);

        // let mut context = "_ctx: dojo::world::Context".to_string();
        // if let Some(first) = elements.first() {
        //     // If context is first, move it to last.
        //     if is_context(db, first) {
        //         let ctx = elements.remove(0);
        //         context = ctx.as_syntax_node().get_text(db);
        //     }
        // } else if let Some(param) = elements.iter().find(|p| is_context(db, p)) {
        //     // Context not the first element, but exists.
        //     self.diagnostics.push(PluginDiagnostic {
        //         message: "Context must be first parameter when provided".into(),
        //         stable_ptr: param.stable_ptr().untyped(),
        //     });
        // }

        let params = elements.iter().map(|e| e.as_syntax_node().get_text(db)).collect::<Vec<_>>();
        // params.push(context);
        let params = params.join(", ");

        let ret_clause = if let ReturnTypeClause(clause) = signature.ret_ty(db) {
            RewriteNode::new_trimmed(clause.as_syntax_node())
        } else {
            RewriteNode::Text("".to_string())
        };

        rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
                #[external(v0)]
                fn execute(self: @ContractState, $params$) $ret_clause$ $body$
            ",
            UnorderedHashMap::from([
                ("params".to_string(), RewriteNode::Text(params)),
                (
                    "body".to_string(),
                    RewriteNode::new_trimmed(function_ast.body(db).as_syntax_node()),
                ),
                ("ret_clause".to_string(), ret_clause),
            ]),
        ));

        rewrite_nodes
    }
}

// fn is_context(db: &dyn SyntaxGroup, param: &Param) -> bool {
//     param.type_clause(db).ty(db).as_syntax_node().get_text(db) == "Context"
// }

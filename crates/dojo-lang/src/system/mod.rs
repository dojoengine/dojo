use std::collections::HashMap;

use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, PluginDiagnostic, PluginGeneratedFile, PluginResult,
};
use cairo_lang_semantic::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_semantic::plugin::DynPluginAuxData;
use cairo_lang_syntax::node::ast::MaybeModuleBody;
use cairo_lang_syntax::node::ast::OptionReturnTypeClause::ReturnTypeClause;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_types::system::Dependency;
use smol_str::SmolStr;

// mod deps;
use crate::plugin::{DojoAuxData, SystemAuxData};

pub struct System {
    diagnostics: Vec<PluginDiagnostic>,
    dependencies: HashMap<smol_str::SmolStr, Dependency>,
}

impl System {
    pub fn from_module(db: &dyn SyntaxGroup, module_ast: ast::ItemModule) -> PluginResult {
        let name = module_ast.name(db).text(db);
        let mut system = System { diagnostics: vec![], dependencies: HashMap::new() };

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
                    use option::OptionTrait;
                    use array::SpanTrait;

                    use dojo::world;
                    use dojo::world::IWorldDispatcher;
                    use dojo::world::IWorldDispatcherTrait;
                    use dojo::database::query::Query;
                    use dojo::database::query::QueryTrait;
                    use dojo::database::query::LiteralIntoQuery;
                    use dojo::database::query::TupleSize1IntoQuery;
                    use dojo::database::query::TupleSize2IntoQuery;
                    use dojo::database::query::TupleSize3IntoQuery;
                    use dojo::database::query::IntoPartitioned;
                    use dojo::database::query::IntoPartitionedQuery;

                    #[storage]
                    struct Storage {}

                    #[external(v0)]
                    fn name(self: @ContractState) -> felt252 {
                        '$name$'
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
                    aux_data: DynGeneratedFileAuxData::new(DynPluginAuxData::new(DojoAuxData {
                        patches: builder.patches,
                        components: vec![],
                        systems: vec![SystemAuxData {
                            name,
                            dependencies: system.dependencies.values().cloned().collect(),
                        }],
                    })),
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

        // Collect all the parameters in a Vec
        let param_nodes: Vec<_> = parameters.elements(db);

        // Check if there is a parameter 'ctx: Context'
        // If yes, make sure it's the first one.
        // If not, add it as the first parameter.
        let mut context = RewriteNode::Text("".to_string());
        match param_nodes
            .iter()
            .position(|p| p.as_syntax_node().get_text(db).trim() == "ctx: Context")
        {
            Some(0) => { /* 'ctx: Context' is already the first parameter, do nothing */ }
            Some(_) => panic!("The first parameter must be 'ctx: Context'"),
            None => {
                // 'ctx: Context' is not found at all, add it as the first parameter
                context = RewriteNode::Text("_ctx: dojo::world::Context,".to_string());
            }
        };

        let ret_clause = if let ReturnTypeClause(clause) = signature.ret_ty(db) {
            RewriteNode::new_trimmed(clause.as_syntax_node())
        } else {
            RewriteNode::Text("".to_string())
        };

        rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
                #[external(v0)]
                fn execute(self: @ContractState, $context$$parameters$) $ret_clause$ {
                    $body$
                }
            ",
            UnorderedHashMap::from([
                ("context".to_string(), context),
                ("parameters".to_string(), RewriteNode::new_trimmed(parameters.as_syntax_node())),
                ("body".to_string(), RewriteNode::new_trimmed(function_ast.as_syntax_node())),
                ("ret_clause".to_string(), ret_clause),
            ]),
        ));

        rewrite_nodes
    }
}

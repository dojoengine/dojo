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
use itertools::Itertools;
use smol_str::SmolStr;

use crate::commands::Command;
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
                            return system.from_function(db, fn_ast.clone());
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

                    #[external(v0)]
                    fn dependencies(self: @ContractState) -> Array<(felt252, bool)> {
                        let mut arr = array::ArrayTrait::new();
                        $dependencies$
                        arr
                    }

                    $body$
                }
                ",
                UnorderedHashMap::from([
                    ("name".to_string(), RewriteNode::Text(name.to_string())),
                    ("body".to_string(), RewriteNode::new_modified(body_nodes)),
                    (
                        "dependencies".to_string(),
                        RewriteNode::new_modified(
                            system
                                .dependencies
                                .iter()
                                .sorted_by(|a, b| a.0.cmp(b.0))
                                .map(|(_, dep): (&smol_str::SmolStr, &Dependency)| {
                                    RewriteNode::interpolate_patched(
                                        "array::ArrayTrait::append(ref arr, ('$name$'.into(), \
                                         $write$));\n",
                                        UnorderedHashMap::from([
                                            (
                                                "name".to_string(),
                                                RewriteNode::Text(dep.name.to_string()),
                                            ),
                                            (
                                                "write".to_string(),
                                                RewriteNode::Text(
                                                    if dep.write { "true" } else { "false" }
                                                        .to_string(),
                                                ),
                                            ),
                                        ]),
                                    )
                                })
                                .collect(),
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

    pub fn from_function(
        &mut self,
        db: &dyn SyntaxGroup,
        function_ast: ast::FunctionWithBody,
    ) -> Vec<RewriteNode> {
        let mut rewrite_nodes = vec![];

        let signature = function_ast.declaration(db).signature(db);

        let body_nodes: Vec<RewriteNode> = function_ast
            .body(db)
            .statements(db)
            .elements(db)
            .iter()
            .flat_map(|statement| self.handle_statement(db, statement.clone()))
            .collect();

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
                ("body".to_string(), RewriteNode::new_modified(body_nodes)),
                ("ret_clause".to_string(), ret_clause),
            ]),
        ));

        rewrite_nodes
    }

    fn handle_statement(
        &mut self,
        db: &dyn SyntaxGroup,
        statement_ast: ast::Statement,
    ) -> Vec<RewriteNode> {
        match statement_ast.clone() {
            ast::Statement::Let(statement_let) => {
                if let ast::Expr::InlineMacro(expr_macro) = statement_let.rhs(db) {
                    if let Some(rewrite_nodes) =
                        self.handle_inline_macro(db, Some(statement_let.pattern(db)), expr_macro)
                    {
                        return rewrite_nodes;
                    }
                }
            }
            ast::Statement::Expr(expr) => {
                if let Some(rewrite_nodes) = self.handle_expr(db, expr.expr(db)) {
                    return rewrite_nodes;
                }
            }
            _ => {}
        }

        vec![RewriteNode::Copied(statement_ast.as_syntax_node())]
    }

    fn handle_expr(&mut self, db: &dyn SyntaxGroup, expr: ast::Expr) -> Option<Vec<RewriteNode>> {
        match expr {
            ast::Expr::If(expr_if) => Some(self.handle_if(db, expr_if, false)),
            ast::Expr::Block(expr_block) => Some(self.handle_block(db, expr_block)),
            ast::Expr::Match(expr_match) => Some(self.handle_match(db, expr_match)),
            ast::Expr::Loop(expr_loop) => Some(self.handle_loop(db, expr_loop)),
            ast::Expr::InlineMacro(expr_macro) => self.handle_inline_macro(db, None, expr_macro),
            _ => None,
        }
    }

    fn handle_inline_macro(
        &mut self,
        db: &dyn SyntaxGroup,
        var_name: Option<ast::Pattern>,
        expr_macro: ast::ExprInlineMacro,
    ) -> Option<Vec<RewriteNode>> {
        let command = Command::try_from_ast(db, var_name, expr_macro);

        match command {
            Some(c) => {
                self.diagnostics.extend(c.diagnostics);
                self.update_deps(c.component_deps);
                Some(c.rewrite_nodes)
            }
            None => None,
        }
    }

    fn handle_if(
        &mut self,
        db: &dyn SyntaxGroup,
        expr_if: ast::ExprIf,
        is_else_if: bool,
    ) -> Vec<RewriteNode> {
        // recurse thru if blocks
        let if_block: Vec<RewriteNode> = self.handle_block(db, expr_if.if_block(db));
        let else_prefix = if is_else_if { "else " } else { "" };
        let code = format!("{else_prefix}if $condition$ $block$");
        let if_rewrite = RewriteNode::interpolate_patched(
            &code,
            UnorderedHashMap::from([
                (
                    "condition".to_string(),
                    RewriteNode::Copied(expr_if.condition(db).as_syntax_node()),
                ),
                ("block".to_string(), RewriteNode::new_modified(if_block)),
            ]),
        );

        // recurse thru else/if blocks
        if let ast::OptionElseClause::ElseClause(else_clause) = expr_if.else_clause(db) {
            match else_clause.else_block_or_if(db) {
                ast::BlockOrIf::Block(expr_else_block) => {
                    let else_block = self.handle_block(db, expr_else_block);
                    let else_rewrite = RewriteNode::interpolate_patched(
                        "else $block$",
                        UnorderedHashMap::from([(
                            "block".to_string(),
                            RewriteNode::new_modified(else_block),
                        )]),
                    );
                    return vec![if_rewrite, else_rewrite];
                }
                ast::BlockOrIf::If(expr_else_if) => {
                    let else_if_nodes: Vec<RewriteNode> = self.handle_if(db, expr_else_if, true);
                    return vec![if_rewrite].into_iter().chain(else_if_nodes.into_iter()).collect();
                }
            };
        }

        vec![if_rewrite]
    }

    fn handle_loop(&mut self, db: &dyn SyntaxGroup, expr_loop: ast::ExprLoop) -> Vec<RewriteNode> {
        let loop_nodes: Vec<RewriteNode> = self.handle_block(db, expr_loop.body(db));
        let loop_rewrite = RewriteNode::interpolate_patched(
            "loop $block$;",
            UnorderedHashMap::from([("block".to_string(), RewriteNode::new_modified(loop_nodes))]),
        );
        vec![loop_rewrite]
    }

    fn handle_block(
        &mut self,
        db: &dyn SyntaxGroup,
        expr_block: ast::ExprBlock,
    ) -> Vec<RewriteNode> {
        let block_nodes: Vec<RewriteNode> = expr_block
            .statements(db)
            .elements(db)
            .iter()
            .flat_map(|statement| self.handle_statement(db, statement.clone()))
            .collect();

        let block_rewrite = RewriteNode::interpolate_patched(
            "{ $nodes$ }",
            UnorderedHashMap::from([("nodes".to_string(), RewriteNode::new_modified(block_nodes))]),
        );
        vec![block_rewrite]
    }

    fn handle_match(
        &mut self,
        db: &dyn SyntaxGroup,
        expr_match: ast::ExprMatch,
    ) -> Vec<RewriteNode> {
        let match_nodes: Vec<RewriteNode> = expr_match
            .arms(db)
            .elements(db)
            .iter()
            .flat_map(|arm| {
                if let ast::Expr::Block(arm_block) = arm.expression(db) {
                    let arm_pat = arm.pattern(db);
                    let arm_block = self.handle_block(db, arm_block);
                    let arm_rewrite = RewriteNode::interpolate_patched(
                        "$pattern$ => $block$,",
                        UnorderedHashMap::from([
                            ("pattern".to_string(), RewriteNode::Copied(arm_pat.as_syntax_node())),
                            ("block".to_string(), RewriteNode::new_modified(arm_block)),
                        ]),
                    );
                    return vec![arm_rewrite];
                }

                vec![RewriteNode::Copied(arm.as_syntax_node())]
            })
            .collect();

        let match_rewrite = RewriteNode::interpolate_patched(
            "match $expr$ { $arms$ }",
            UnorderedHashMap::from([
                ("expr".to_string(), RewriteNode::Copied(expr_match.expr(db).as_syntax_node())),
                ("arms".to_string(), RewriteNode::new_modified(match_nodes)),
            ]),
        );
        vec![match_rewrite]
    }

    fn update_deps(&mut self, deps: Vec<Dependency>) {
        for dep in deps {
            if let Some(existing) = self.dependencies.get(&SmolStr::from(dep.name.as_str())) {
                self.dependencies
                    .insert(dep.name.clone().into(), merge_deps(dep.clone(), existing.clone()));
            } else {
                self.dependencies.insert(dep.name.clone().into(), dep.clone());
            }
        }
    }
}

fn merge_deps(a: Dependency, b: Dependency) -> Dependency {
    Dependency { name: a.name, read: a.read || b.read, write: a.write || b.write }
}

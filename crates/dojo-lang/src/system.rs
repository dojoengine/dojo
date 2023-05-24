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

use crate::commands::Command;
use crate::plugin::{DojoAuxData, SystemAuxData};

pub struct System {
    diagnostics: Vec<PluginDiagnostic>,
    dependencies: Vec<smol_str::SmolStr>,
}

impl System {
    pub fn from_module(db: &dyn SyntaxGroup, module_ast: ast::ItemModule) -> PluginResult {
        let name = module_ast.name(db).text(db);
        let mut system = System { diagnostics: vec![], dependencies: vec![] };

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
                #[contract]
                mod $name$System {
                    use option::OptionTrait;
                    use array::SpanTrait;

                    use dojo_core::world;
                    use dojo_core::interfaces::IWorldDispatcher;
                    use dojo_core::interfaces::IWorldDispatcherTrait;
                    use dojo_core::storage::query::Query;
                    use dojo_core::storage::query::QueryTrait;
                    use dojo_core::storage::query::LiteralIntoQuery;
                    use dojo_core::storage::query::TupleSize1IntoQuery;
                    use dojo_core::storage::query::TupleSize2IntoQuery;
                    use dojo_core::storage::query::TupleSize3IntoQuery;
                    use dojo_core::storage::query::IntoPartitioned;
                    use dojo_core::storage::query::IntoPartitionedQuery;

                    #[view]
                    fn name() -> dojo_core::string::ShortString {
                        dojo_core::string::ShortStringTrait::new('$name$')
                    }

                    $body$
                }
                ",
                HashMap::from([
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
                            name: format!("{name}System").into(),
                            dependencies: system.dependencies.clone(),
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
        let separator = if parameters.elements(db).is_empty() { "" } else { ", " };
        let ret_clause = if let ReturnTypeClause(clause) = signature.ret_ty(db) {
            RewriteNode::new_trimmed(clause.as_syntax_node())
        } else {
            RewriteNode::Text("".to_string())
        };

        rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
                #[external]
                fn execute($parameters$$separator$world_address: starknet::ContractAddress) \
             $ret_clause$ {
                    $body$
                }
            ",
            HashMap::from([
                ("parameters".to_string(), RewriteNode::new_trimmed(parameters.as_syntax_node())),
                ("separator".to_string(), RewriteNode::Text(separator.to_string())),
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
                if let ast::Expr::FunctionCall(expr_fn) = statement_let.rhs(db) {
                    if let Some(rewrite_nodes) =
                        self.handle_fn_call(db, Some(statement_let.pattern(db)), expr_fn)
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
            ast::Expr::FunctionCall(expr_fn) => self.handle_fn_call(db, None, expr_fn),
            ast::Expr::If(expr_if) => Some(self.handle_if(db, expr_if, false)),
            ast::Expr::Block(expr_block) => Some(self.handle_block(db, expr_block)),
            ast::Expr::Match(expr_match) => Some(self.handle_match(db, expr_match)),
            ast::Expr::Loop(expr_loop) => Some(self.handle_loop(db, expr_loop)),
            _ => None,
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
            HashMap::from([
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
                        HashMap::from([(
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
            HashMap::from([("block".to_string(), RewriteNode::new_modified(loop_nodes))]),
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
            HashMap::from([("nodes".to_string(), RewriteNode::new_modified(block_nodes))]),
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
                        HashMap::from([
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
            HashMap::from([
                ("expr".to_string(), RewriteNode::Copied(expr_match.expr(db).as_syntax_node())),
                ("arms".to_string(), RewriteNode::new_modified(match_nodes)),
            ]),
        );
        vec![match_rewrite]
    }

    fn handle_fn_call(
        &mut self,
        db: &dyn SyntaxGroup,
        var_name: Option<ast::Pattern>,
        expr_fn: ast::ExprFunctionCall,
    ) -> Option<Vec<RewriteNode>> {
        let elements = expr_fn.path(db).elements(db);
        let segment = elements.first().unwrap();
        match segment {
            ast::PathSegment::WithGenericArgs(segment_genric) => {
                if segment_genric.ident(db).text(db).as_str() == "commands" {
                    let command = Command::from_ast(db, var_name, expr_fn);
                    self.diagnostics.extend(command.diagnostics);
                    self.dependencies.extend(command.component_deps);
                    return Some(command.rewrite_nodes);
                }
            }
            ast::PathSegment::Simple(segment_simple) => {
                if segment_simple.ident(db).text(db).as_str() == "commands" {
                    let command = Command::from_ast(db, var_name, expr_fn);
                    self.diagnostics.extend(command.diagnostics);
                    self.dependencies.extend(command.component_deps);
                    return Some(command.rewrite_nodes);
                }
            }
        }

        None
    }
}

use std::collections::{HashMap, HashSet};

use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use dojo_project::WorldConfig;
use smol_str::SmolStr;

use crate::plugin::get_contract_address;

pub struct Spawn {
    world_config: WorldConfig,
    entity_id: RewriteNode,
    pub dependencies: HashSet<SmolStr>,
    pub body_nodes: Vec<RewriteNode>,
}

impl Spawn {
    pub fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: ast::Pattern,
        spawn_ast: ast::ExprFunctionCall,
        world_config: WorldConfig,
    ) -> Self {
        let mut spawn = Spawn {
            world_config,
            entity_id: RewriteNode::new_trimmed(let_pattern.as_syntax_node()),
            dependencies: HashSet::new(),
            body_nodes: vec![],
        };

        spawn.body_nodes.push(RewriteNode::interpolate_patched(
            "let $entity_id$ = IWorldDispatcher { contract_address: world_address \
             }.issue_entity(starknet::get_caller_address());",
            HashMap::from([("entity_id".to_string(), spawn.entity_id.clone())]),
        ));

        if let Some(arg) = spawn_ast.arguments(db).args(db).elements(db).first() {
            if let ast::ArgClause::Unnamed(clause) = arg.arg_clause(db) {
                match clause.value(db) {
                    ast::Expr::Parenthesized(bundle) => {
                        spawn.handle_struct(db, bundle.expr(db));
                    }
                    ast::Expr::Tuple(tuple) => {
                        for expr in tuple.expressions(db).elements(db) {
                            spawn.handle_struct(db, expr);
                        }
                    }
                    _ => {}
                }
            }
        }

        spawn
    }

    fn handle_struct(&mut self, db: &dyn SyntaxGroup, expr: ast::Expr) {
        if let ast::Expr::StructCtorCall(ctor) = expr {
            if let Some(ast::PathSegment::Simple(segment)) = ctor.path(db).elements(db).last() {
                let component = segment.ident(db).text(db);
                let component_address = format!(
                    "{:#x}",
                    get_contract_address(
                        component.as_str(),
                        self.world_config.initializer_class_hash.unwrap_or_default(),
                        self.world_config.address.unwrap_or_default(),
                    )
                );

                self.body_nodes.push(RewriteNode::interpolate_patched(
                    "I$component$Dispatcher { contract_address: \
                     starknet::contract_address_const::<$component_address$>() }.set($entity_id$, \
                     $ctor$);
                    ",
                    HashMap::from([
                        ("component".to_string(), RewriteNode::Text(component.to_string())),
                        ("component_address".to_string(), RewriteNode::Text(component_address)),
                        ("ctor".to_string(), RewriteNode::new_trimmed(ctor.as_syntax_node())),
                        ("entity_id".to_string(), self.entity_id.clone()),
                    ]),
                ));

                self.dependencies.extend([
                    SmolStr::from(format!("I{}Dispatcher", component)),
                    SmolStr::from(format!("I{}DispatcherTrait", component)),
                ]);
            }
        }
    }
}

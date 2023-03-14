use std::collections::{HashMap, HashSet};

use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use dojo_project::WorldConfig;
use smol_str::SmolStr;

use crate::plugin::get_contract_address;

pub struct Spawn {
    world_config: WorldConfig,
    entity_id_name: SmolStr,
    pub dependencies: HashSet<SmolStr>,
    pub body_nodes: Vec<RewriteNode>,
}

impl Spawn {
    pub fn handle_spawn(
        db: &dyn SyntaxGroup,
        spawn_ast: ast::ExprFunctionCall,
        world_config: WorldConfig,
    ) -> Self {
        let mut spawn = Spawn {
            world_config,
            // TODO: Make entity id var unique
            entity_id_name: SmolStr::new("entity_id"),
            dependencies: HashSet::new(),
            body_nodes: vec![],
        };

        spawn.body_nodes.push(RewriteNode::interpolate_patched(
            "let owner = starknet::get_caller_address();
                let $entity_id$ = IWorldDispatcher { contract_address: world_address \
             }.issue_entity(owner);",
            HashMap::from([(
                "entity_id".to_string(),
                RewriteNode::Text(spawn.entity_id_name.to_string()),
            )]),
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
                        (
                            "entity_id".to_string(),
                            RewriteNode::Text(self.entity_id_name.to_string()),
                        ),
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

use std::collections::{HashMap, HashSet};

use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use dojo_project::WorldConfig;
use smol_str::SmolStr;

use crate::plugin::get_contract_address;

pub fn handle_spawn(
    db: &dyn SyntaxGroup,
    spawn_ast: ast::ExprFunctionCall,
    world_config: WorldConfig,
) -> (HashSet<SmolStr>, Vec<RewriteNode>) {
    let mut dependencies = HashSet::new();
    // TODO: Make entity id var unique
    let mut body_nodes = vec![RewriteNode::Text(
        "let owner = starknet::get_caller_address();
            let entity_id = IWorldDispatcher { contract_address: world_address \
         }.issue_entity(owner);"
            .to_string(),
    )];

    if let Some(arg) = spawn_ast.arguments(db).args(db).elements(db).first() {
        if let ast::ArgClause::Unnamed(clause) = arg.arg_clause(db) {
            match clause.value(db) {
                ast::Expr::Parenthesized(bundle) => {
                    let (deps, nodes) = handle_struct(db, bundle.expr(db), world_config);
                    dependencies.extend(deps);
                    body_nodes.extend(nodes);
                }
                ast::Expr::Tuple(tuple) => {
                    for expr in tuple.expressions(db).elements(db) {
                        let (deps, nodes) = handle_struct(db, expr, world_config);
                        dependencies.extend(deps);
                        body_nodes.extend(nodes);
                    }
                }
                _ => {}
            }
        }
    }

    (dependencies, body_nodes)
}

fn handle_struct(
    db: &dyn SyntaxGroup,
    expr: ast::Expr,
    world_config: WorldConfig,
) -> (HashSet<SmolStr>, Vec<RewriteNode>) {
    let mut dependencies = HashSet::new();
    let mut body_nodes = vec![];

    if let ast::Expr::StructCtorCall(ctor) = expr {
        if let Some(ast::PathSegment::Simple(segment)) = ctor.path(db).elements(db).last() {
            let component = segment.ident(db).text(db);
            let component_address = format!(
                "{:#x}",
                get_contract_address(
                    component.as_str(),
                    world_config.initializer_class_hash.unwrap_or_default(),
                    world_config.address.unwrap_or_default(),
                )
            );

            body_nodes.push(RewriteNode::interpolate_patched(
                "I$component$Dispatcher { contract_address: \
                 starknet::contract_address_const::<$component_address$>() }.set(entity_id, \
                 $ctor$);
                ",
                HashMap::from([
                    ("component".to_string(), RewriteNode::Text(component.to_string())),
                    ("component_address".to_string(), RewriteNode::Text(component_address)),
                    ("ctor".to_string(), RewriteNode::new_trimmed(ctor.as_syntax_node())),
                ]),
            ));

            dependencies.extend([
                component.clone(),
                SmolStr::from(format!("I{}Dispatcher", component)),
                SmolStr::from(format!("I{}DispatcherTrait", component)),
            ]);
        }
    }

    (dependencies, body_nodes)
}

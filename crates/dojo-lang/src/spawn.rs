use std::collections::{HashMap, HashSet};

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use dojo_project::WorldConfig;
use smol_str::SmolStr;

use crate::path::expand_path;
use crate::plugin::get_contract_address;

pub struct Spawn {
    world_config: WorldConfig,
    pub dependencies: HashSet<SmolStr>,
    pub rewrite_nodes: Vec<RewriteNode>,
    pub diagnostics: Vec<PluginDiagnostic>,
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
            dependencies: HashSet::new(),
            rewrite_nodes: vec![],
            diagnostics: vec![],
        };

        if let ast::PathSegment::Simple(method) = spawn_ast.path(db).elements(db).last().unwrap() {
            match method.ident(db).text(db).as_str() {
                "bundle" => {
                    let elements = spawn_ast.arguments(db).args(db).elements(db);

                    if elements.len() != 2 {
                        spawn.diagnostics.push(PluginDiagnostic {
                            message: "Invalid arguements. Expected \"(entity_path, components)\""
                                .to_string(),
                            stable_ptr: spawn_ast.arguments(db).as_syntax_node().stable_ptr(),
                        });
                        return spawn;
                    }

                    match expand_path(db, elements.first().unwrap().clone(), 4) {
                        Ok(entity_path) => {
                            let bundle = elements.last().unwrap();
                            if let ast::ArgClause::Unnamed(clause) = bundle.arg_clause(db) {
                                match clause.value(db) {
                                    ast::Expr::Parenthesized(bundle) => {
                                        spawn.handle_struct(db, entity_path, bundle.expr(db));
                                    }
                                    ast::Expr::Tuple(tuple) => {
                                        for expr in tuple.expressions(db).elements(db) {
                                            spawn.handle_struct(db, entity_path.clone(), expr);
                                        }
                                    }
                                    _ => {
                                        spawn.diagnostics.push(PluginDiagnostic {
                                            message: "Invalid entity id. Expected \"(...)\""
                                                .to_string(),
                                            stable_ptr: clause.as_syntax_node().stable_ptr(),
                                        });
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            spawn.diagnostics.push(err);
                        }
                    }
                }
                "entity" => {
                    let elements = spawn_ast.arguments(db).args(db).elements(db);

                    match expand_path(db, elements.first().unwrap().clone(), 3) {
                        Ok(entity_path) => {
                            spawn.rewrite_nodes.push(RewriteNode::interpolate_patched(
                                "let $entity_id$ = IWorldDispatcher { contract_address: \
                                 world_address }.next_entity_id(($entity_path$));
                                ",
                                HashMap::from([
                                    (
                                        "entity_id".to_string(),
                                        RewriteNode::new_trimmed(let_pattern.as_syntax_node()),
                                    ),
                                    ("entity_path".to_string(), entity_path),
                                ]),
                            ));
                        }
                        Err(err) => {
                            spawn.diagnostics.push(err);
                        }
                    }
                }
                _ => {}
            }
        }

        spawn
    }

    fn handle_struct(&mut self, db: &dyn SyntaxGroup, entity_id: RewriteNode, expr: ast::Expr) {
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

                self.rewrite_nodes.push(RewriteNode::interpolate_patched(
                    "I$component$Dispatcher { contract_address: \
                     starknet::contract_address_const::<$component_address$>() }.set($entity_id$, \
                     $ctor$);
                    ",
                    HashMap::from([
                        ("component".to_string(), RewriteNode::Text(component.to_string())),
                        ("component_address".to_string(), RewriteNode::Text(component_address)),
                        ("ctor".to_string(), RewriteNode::new_trimmed(ctor.as_syntax_node())),
                        ("entity_id".to_string(), entity_id),
                    ]),
                ));

                // TODO: Figure out how to automatically resolve dispatcher dependencies.
                // self.dependencies.extend([
                //     SmolStr::from(format!("I{}Dispatcher", component)),
                //     SmolStr::from(format!("I{}DispatcherTrait", component)),
                // ]);
            }
        }
    }
}

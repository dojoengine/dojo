use std::collections::{HashMap, HashSet};

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use dojo_project::WorldConfig;
use smol_str::SmolStr;

use crate::plugin::get_contract_address;

pub struct Command {
    world_config: WorldConfig,
    pub dependencies: HashSet<SmolStr>,
    pub rewrite_nodes: Vec<RewriteNode>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

impl Command {
    pub fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: ast::Pattern,
        command_ast: ast::ExprFunctionCall,
        world_config: WorldConfig,
    ) -> Self {
        let mut command = Command {
            world_config,
            dependencies: HashSet::new(),
            rewrite_nodes: vec![],
            diagnostics: vec![],
        };

        if let ast::PathSegment::Simple(method) = command_ast.path(db).elements(db).last().unwrap()
        {
            match method.ident(db).text(db).as_str() {
                "spawn" => {
                    let elements = command_ast.arguments(db).args(db).elements(db);

                    if elements.len() != 2 {
                        command.diagnostics.push(PluginDiagnostic {
                            message: "Invalid arguements. Expected \"(storage_key, \
                                      (components,))\""
                                .to_string(),
                            stable_ptr: command_ast.arguments(db).as_syntax_node().stable_ptr(),
                        });
                        return command;
                    }
                    let storage_key = elements.first().unwrap();
                    command.rewrite_nodes.push(RewriteNode::interpolate_patched(
                        "let __$var_name$_sk: dojo::storage::StorageKey = $storage_key$;
                        let __$var_name$_sk_id = __$var_name$_sk.id();
                        ",
                        HashMap::from([
                            (
                                "var_name".to_string(),
                                RewriteNode::new_trimmed(let_pattern.as_syntax_node()),
                            ),
                            (
                                "storage_key".to_string(),
                                RewriteNode::new_trimmed(storage_key.as_syntax_node()),
                            ),
                        ]),
                    ));

                    let bundle = elements.last().unwrap();
                    if let ast::ArgClause::Unnamed(clause) = bundle.arg_clause(db) {
                        match clause.value(db) {
                            ast::Expr::Parenthesized(bundle) => {
                                command.handle_struct(db, let_pattern, bundle.expr(db));
                            }
                            ast::Expr::Tuple(tuple) => {
                                for expr in tuple.expressions(db).elements(db) {
                                    command.handle_struct(db, let_pattern.clone(), expr);
                                }
                            }
                            _ => {
                                command.diagnostics.push(PluginDiagnostic {
                                    message: "Invalid storage key. Expected \"(...)\"".to_string(),
                                    stable_ptr: clause.as_syntax_node().stable_ptr(),
                                });
                            }
                        }
                    }
                }
                "uuid" => {
                    command.rewrite_nodes.push(RewriteNode::interpolate_patched(
                        "let $var_name$ = IWorldDispatcher { contract_address: world_address \
                         }.uuid();
                                ",
                        HashMap::from([(
                            "var_name".to_string(),
                            RewriteNode::new_trimmed(let_pattern.as_syntax_node()),
                        )]),
                    ));
                }
                _ => {}
            }
        }

        command
    }

    fn handle_struct(&mut self, db: &dyn SyntaxGroup, var_name: ast::Pattern, expr: ast::Expr) {
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
                     starknet::contract_address_const::<$component_address$>() \
                     }.set(__$var_name$_sk_id, $ctor$);
                    ",
                    HashMap::from([
                        ("component".to_string(), RewriteNode::Text(component.to_string())),
                        ("component_address".to_string(), RewriteNode::Text(component_address)),
                        ("ctor".to_string(), RewriteNode::new_trimmed(ctor.as_syntax_node())),
                        (
                            "var_name".to_string(),
                            RewriteNode::new_trimmed(var_name.as_syntax_node()),
                        ),
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

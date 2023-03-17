use std::collections::{HashMap, HashSet};

use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use dojo_project::WorldConfig;
use itertools::Itertools;
use sanitizer::StringSanitizer;
use smol_str::SmolStr;

use crate::plugin::get_contract_address;

pub struct Query {
    query_id: String,
    query_pattern: String,
    world_config: WorldConfig,
    components: Vec<SmolStr>,
    pub dependencies: HashSet<SmolStr>,
    pub body_nodes: Vec<RewriteNode>,
}

impl Query {
    pub fn from_ast(
        db: &dyn SyntaxGroup,
        world_config: WorldConfig,
        let_pattern: ast::Pattern,
        query_ast: ast::ExprFunctionCall,
        generics_segment: ast::PathSegmentWithGenericArgs,
    ) -> Self {
        let mut query_id = StringSanitizer::from(let_pattern.as_syntax_node().get_text(db));
        query_id.to_snake_case();
        let mut query = Query {
            world_config,
            query_id: query_id.get(),
            query_pattern: let_pattern.as_syntax_node().get_text(db),
            components: vec![],
            dependencies: HashSet::new(),
            body_nodes: vec![],
        };

        for arg in generics_segment.generic_args(db).generic_args(db).elements(db) {
            if let ast::GenericArg::Expr(expr) = arg {
                query.find_components(db, expr.value(db));
            }
        }

        if let ast::PathSegment::Simple(el) = query_ast.path(db).elements(db).last().unwrap() {
            match el.ident(db).text(db).as_str() {
                "ids" => {
                    query.rewrite_ids_query(db, query_ast);
                }
                "entity" => {
                    query.rewrite_entity_query(db, query_ast);
                }
                _ => todo!(),
            }
        }

        query
    }

    pub fn rewrite_ids_query(&mut self, db: &dyn SyntaxGroup, query_ast: ast::ExprFunctionCall) {
        let elements = query_ast.arguments(db).args(db).elements(db);

        let mut entity_path: Vec<String> = vec!["0".to_string(); 3];
        if !elements.is_empty() {
            let entity_id = elements.first().unwrap();
            if let ast::ArgClause::Unnamed(path) = entity_id.arg_clause(db) {
                match path.value(db) {
                    ast::Expr::Parenthesized(bundle) => {
                        entity_path[2] = bundle.expr(db).as_syntax_node().get_text(db);
                    }
                    ast::Expr::Tuple(tuple) => {
                        let mut elements = tuple.expressions(db).elements(db);
                        elements.reverse();
                        let mut i = elements.len() - 1;
                        for expr in elements {
                            entity_path[i] = expr.as_syntax_node().get_text(db);
                            i -= 1;
                        }
                    }
                    _ => {}
                }
            }
        }

        self.body_nodes.push(RewriteNode::interpolate_patched(
            "let $query_pattern$ = ArrayTrait::<usize>::new();",
            HashMap::from([(
                "query_pattern".to_string(),
                RewriteNode::Text(self.query_id.clone()),
            )]),
        ));
        self.body_nodes.extend(
            self.components
                .iter()
                .map(|component| {
                    let component_address = format!(
                        "{:#x}",
                        get_contract_address(
                            component.as_str(),
                            self.world_config.initializer_class_hash.unwrap_or_default(),
                            self.world_config.address.unwrap_or_default(),
                        )
                    );
                    RewriteNode::interpolate_patched(
                        "
                        let $query_id$_$query_subtype$_ids = IWorldDispatcher { contract_address: \
                         world_address \
                         }.get_entities(starknet::contract_address_const::<$component_address$>(), \
                         ($entity_path$));
                        ",
                        HashMap::from([
                            (
                                "query_subtype".to_string(),
                                RewriteNode::Text(component.to_string().to_ascii_lowercase()),
                            ),
                            ("query_id".to_string(), RewriteNode::Text(self.query_id.clone())),
                            ("entity_path".to_string(), RewriteNode::Text(entity_path.join(", "))),
                            ("component_address".to_string(), RewriteNode::Text(component_address)),
                        ]),
                    )
                })
                .collect::<Vec<_>>(),
        );
    }

    pub fn rewrite_entity_query(&mut self, db: &dyn SyntaxGroup, query_ast: ast::ExprFunctionCall) {
        let elements = query_ast.arguments(db).args(db).elements(db);
        let entity_id = elements.first().unwrap();

        let mut entity_path: Vec<String> = vec!["0".to_string(); 4];
        if let ast::ArgClause::Unnamed(path) = entity_id.arg_clause(db) {
            match path.value(db) {
                ast::Expr::Parenthesized(bundle) => {
                    entity_path[3] = bundle.expr(db).as_syntax_node().get_text(db);
                }
                ast::Expr::Tuple(tuple) => {
                    let mut elements = tuple.expressions(db).elements(db);
                    elements.reverse();
                    let elements_len = elements.len();
                    for (count, expr) in elements.into_iter().enumerate() {
                        let index = elements_len - 1 - count;
                        entity_path[index] = expr.as_syntax_node().get_text(db);
                    }
                }
                _ => {}
            }
        }

        let part_names = self
            .components
            .iter()
            .map(|component| {
                format!(
                    "{query_id}_{query_subtype}",
                    query_id = self.query_id,
                    query_subtype = component.to_string().to_ascii_lowercase()
                )
            })
            .join(", ");

        for component in self.components.iter() {
            let component_address = format!(
                "{:#x}",
                get_contract_address(
                    component.as_str(),
                    self.world_config.initializer_class_hash.unwrap_or_default(),
                    self.world_config.address.unwrap_or_default(),
                )
            );
            self.body_nodes.push(RewriteNode::interpolate_patched(
                "
                let $query_id$_$query_subtype$ = I$component$Dispatcher { contract_address: \
                 starknet::contract_address_const::<$component_address$>() }.get(($entity_path$));
                ",
                HashMap::from([
                    ("component".to_string(), RewriteNode::Text(component.to_string())),
                    (
                        "query_subtype".to_string(),
                        RewriteNode::Text(component.to_string().to_ascii_lowercase()),
                    ),
                    ("query_id".to_string(), RewriteNode::Text(self.query_id.clone())),
                    ("entity_path".to_string(), RewriteNode::Text(entity_path.join(", "))),
                    ("component_address".to_string(), RewriteNode::Text(component_address)),
                ]),
            ));

            // TODO: Figure out how to automatically resolve dispatcher dependencies.
            // self.dependencies.extend([
            //     SmolStr::from(format!("I{}Dispatcher", component)),
            //     SmolStr::from(format!("I{}DispatcherTrait", component)),
            // ]);
        }

        if self.components.len() > 1 {
            self.body_nodes.push(RewriteNode::interpolate_patched(
                "let $query_pattern$ = ($part_names$);
                ",
                HashMap::from([
                    ("query_pattern".to_string(), RewriteNode::Text(self.query_pattern.clone())),
                    ("part_names".to_string(), RewriteNode::Text(part_names)),
                ]),
            ));
        } else {
            self.body_nodes.push(RewriteNode::interpolate_patched(
                "let $query_pattern$ = $part_names$;
                ",
                HashMap::from([
                    ("query_pattern".to_string(), RewriteNode::Text(self.query_pattern.clone())),
                    ("part_names".to_string(), RewriteNode::Text(part_names)),
                ]),
            ));
        }
    }

    fn find_components(&mut self, db: &dyn SyntaxGroup, expression: ast::Expr) {
        match expression {
            ast::Expr::Tuple(tuple) => {
                for element in tuple.expressions(db).elements(db) {
                    self.find_components(db, element);
                }
            }
            ast::Expr::Parenthesized(parenthesized) => {
                self.find_components(db, parenthesized.expr(db))
            }
            ast::Expr::Path(path) => match path.elements(db).last().unwrap() {
                ast::PathSegment::WithGenericArgs(segment) => {
                    let generic = segment.generic_args(db);

                    for param in generic.generic_args(db).elements(db) {
                        if let ast::GenericArg::Expr(expr) = param {
                            self.find_components(db, expr.value(db));
                        }
                    }
                }
                ast::PathSegment::Simple(segment) => {
                    self.components.push(segment.ident(db).text(db));
                }
            },
            _ => {
                unimplemented!(
                    "Unsupported expression type: {}",
                    expression.as_syntax_node().get_text(db)
                );
            }
        }
    }
}

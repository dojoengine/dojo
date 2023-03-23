use std::collections::{HashMap, HashSet};

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use itertools::Itertools;
use sanitizer::StringSanitizer;
use smol_str::SmolStr;

pub struct Query {
    query_id: String,
    query_pattern: String,
    components: Vec<SmolStr>,
    pub dependencies: HashSet<SmolStr>,
    pub diagnostics: Vec<PluginDiagnostic>,
    pub rewrite_nodes: Vec<RewriteNode>,
}

impl Query {
    pub fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        query_ast: ast::ExprFunctionCall,
    ) -> Self {
        let mut query_id = StringSanitizer::from(let_pattern.as_syntax_node().get_text(db));
        query_id.to_snake_case();
        let mut query = Query {
            query_id: query_id.get(),
            query_pattern: let_pattern.as_syntax_node().get_text(db),
            components: vec![],
            dependencies: HashSet::new(),
            diagnostics: vec![],
            rewrite_nodes: vec![],
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

    pub fn rewrite_entity_query(&mut self, db: &dyn SyntaxGroup, query_ast: ast::ExprFunctionCall) {
        let elements = query_ast.arguments(db).args(db).elements(db);
        let storage_key = elements.first().unwrap();

        let part_names = self
            .components
            .iter()
            .map(|component| {
                format!(
                    "__{query_id}_{query_subtype}",
                    query_id = self.query_id,
                    query_subtype = component.to_string().to_ascii_lowercase()
                )
            })
            .join(", ");

        for component in self.components.iter() {
            self.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "
                let mut __$query_id$_$query_subtype$_raw = IWorldDispatcher {
                    contract_address: world_address
                }.read('$component$', $storage_key$, 0_u8, 0_usize);
                let __$query_id$_$query_subtype$ = serde::Serde::<$component$>::deserialize(
                    ref __$query_id$_$query_subtype$_raw
                );
                ",
                HashMap::from([
                    ("component".to_string(), RewriteNode::Text(component.to_string())),
                    (
                        "query_subtype".to_string(),
                        RewriteNode::Text(component.to_string().to_ascii_lowercase()),
                    ),
                    ("query_id".to_string(), RewriteNode::Text(self.query_id.clone())),
                    (
                        "storage_key".to_string(),
                        RewriteNode::new_trimmed(storage_key.as_syntax_node()),
                    ),
                ]),
            ));
        }

        if self.components.len() > 1 {
            self.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "let $query_pattern$ = ($part_names$);
                ",
                HashMap::from([
                    ("query_pattern".to_string(), RewriteNode::Text(self.query_pattern.clone())),
                    ("part_names".to_string(), RewriteNode::Text(part_names)),
                ]),
            ));
        } else {
            self.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "let $query_pattern$ = $part_names$;
                ",
                HashMap::from([
                    ("query_pattern".to_string(), RewriteNode::Text(self.query_pattern.clone())),
                    ("part_names".to_string(), RewriteNode::Text(part_names)),
                ]),
            ));
        }
    }  
}

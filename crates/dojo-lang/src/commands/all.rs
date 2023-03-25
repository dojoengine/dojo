use std::collections::HashMap;

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use sanitizer::StringSanitizer;
use smol_str::SmolStr;

use super::{CommandData, CommandTrait};

pub struct AllCommand {
    query_id: String,
    data: CommandData,
}

impl CommandTrait for AllCommand {
    fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        command_ast: ast::ExprFunctionCall,
    ) -> Self {
        let mut query_id =
            StringSanitizer::from(let_pattern.unwrap().as_syntax_node().get_text(db));
        query_id.to_snake_case();
        let mut command = AllCommand { query_id: query_id.get(), data: CommandData::new() };

        let partition =
            if let Some(partition) = command_ast.arguments(db).args(db).elements(db).first() {
                RewriteNode::new_trimmed(partition.as_syntax_node())
            } else {
                RewriteNode::Text("0".to_string())
            };

        command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "let $query_pattern$ = ArrayTrait::<usize>::new();",
            HashMap::from([(
                "query_pattern".to_string(),
                RewriteNode::Text(command.query_id.clone()),
            )]),
        ));
        command.data.rewrite_nodes.extend(
            find_components(db, command_ast)
                .iter()
                .map(|component| {
                    RewriteNode::interpolate_patched(
                        "
                        let __$query_id$_$query_subtype$_ids = IWorldDispatcher { \
                         contract_address: world_address }.all('$component$', $partition$);
                        ",
                        HashMap::from([
                            (
                                "query_subtype".to_string(),
                                RewriteNode::Text(component.to_string().to_ascii_lowercase()),
                            ),
                            ("query_id".to_string(), RewriteNode::Text(command.query_id.clone())),
                            ("partition".to_string(), partition.clone()),
                            ("component".to_string(), RewriteNode::Text(component.to_string())),
                        ]),
                    )
                })
                .collect::<Vec<_>>(),
        );

        command
    }

    fn rewrite_nodes(&self) -> Vec<RewriteNode> {
        self.data.rewrite_nodes.clone()
    }

    fn diagnostics(&self) -> Vec<PluginDiagnostic> {
        self.data.diagnostics.clone()
    }
}

pub fn find_components(db: &dyn SyntaxGroup, command_ast: ast::ExprFunctionCall) -> Vec<SmolStr> {
    let mut components = vec![];
    if let ast::PathSegment::WithGenericArgs(generic) =
        command_ast.path(db).elements(db).first().unwrap()
    {
        for arg in generic.generic_args(db).generic_args(db).elements(db) {
            if let ast::GenericArg::Expr(expr) = arg {
                components.extend(find_components_inner(db, expr.value(db)));
            }
        }
    }
    components
}

fn find_components_inner(db: &dyn SyntaxGroup, expression: ast::Expr) -> Vec<SmolStr> {
    let mut components = vec![];
    match expression {
        ast::Expr::Tuple(tuple) => {
            for element in tuple.expressions(db).elements(db) {
                components.extend(find_components_inner(db, element));
            }
        }
        ast::Expr::Parenthesized(parenthesized) => {
            components.extend(find_components_inner(db, parenthesized.expr(db)));
        }
        ast::Expr::Path(path) => match path.elements(db).last().unwrap() {
            ast::PathSegment::WithGenericArgs(segment) => {
                let generic = segment.generic_args(db);

                for param in generic.generic_args(db).elements(db) {
                    if let ast::GenericArg::Expr(expr) = param {
                        components.extend(find_components_inner(db, expr.value(db)));
                    }
                }
            }
            ast::PathSegment::Simple(segment) => {
                components.push(segment.ident(db).text(db));
            }
        },
        _ => {
            unimplemented!(
                "Unsupported expression type: {}",
                expression.as_syntax_node().get_text(db)
            );
        }
    }

    components
}

use std::collections::HashMap;

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use sanitizer::StringSanitizer;
use smol_str::SmolStr;

use super::{CommandData, CommandTrait, CAIRO_ERR_MSG_LEN};

pub struct EntitiesCommand {
    query_id: String,
    data: CommandData,
}

impl CommandTrait for EntitiesCommand {
    fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        command_ast: ast::ExprFunctionCall,
    ) -> Self {
        let mut query_id =
            StringSanitizer::from(let_pattern.unwrap().as_syntax_node().get_text(db));
        query_id.to_snake_case();
        let mut command = EntitiesCommand { query_id: query_id.get(), data: CommandData::new() };

        let partition =
            if let Some(partition) = command_ast.arguments(db).args(db).elements(db).first() {
                RewriteNode::new_trimmed(partition.as_syntax_node())
            } else {
                RewriteNode::Text("0".to_string())
            };

        command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
            let mut __$query_id$_ids: Array<Span<dojo_core::integer::u250>> = ArrayTrait::new();
            let mut __$query_id$_entities_raw: Array<Span<Span<felt252>>> = ArrayTrait::new();

            ",
            HashMap::from([("query_id".to_string(), RewriteNode::Text(command.query_id.clone()))]),
        ));

        command.data.rewrite_nodes.extend(
            find_components(db, &command_ast)
                .iter()
                .map(|component| {
                    RewriteNode::interpolate_patched(
                        "
                        let (__$query_id$_$query_subtype$_ids, __$query_id$_$query_subtype$_raw) = \
                         IWorldDispatcher { contract_address: world_address
                        }.entities(dojo_core::string::ShortStringTrait::new('$component$'), \
                         dojo_core::integer::u250Trait::new($partition$));
                        __$query_id$_ids.append(__$query_id$_$query_subtype$_ids);
                        __$query_id$_entities_raw.append(__$query_id$_$query_subtype$_raw);
                        ",
                        HashMap::from([
                            ("query_id".to_string(), RewriteNode::Text(command.query_id.clone())),
                            (
                                "query_subtype".to_string(),
                                RewriteNode::Text(component.to_string().to_ascii_lowercase()),
                            ),
                            ("partition".to_string(), partition.clone()),
                            ("component".to_string(), RewriteNode::Text(component.to_string())),
                        ]),
                    )
                })
                .collect::<Vec<_>>(),
        );

        command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
            let mut __$query_id$_matching_entities = dojo_core::storage::utils::find_matching(
                __$query_id$_ids.span(), __$query_id$_entities_raw.span()
            );

            ",
            HashMap::from([("query_id".to_string(), RewriteNode::Text(command.query_id.clone()))]),
        ));

        command.data.rewrite_nodes.extend(
            find_components(db, &command_ast)
                .iter()
                .enumerate()
                .map(|(idx, component)| {
                    let mut deser_err_msg =
                        format!("{} failed to deserialize", component.to_string());
                    deser_err_msg.truncate(CAIRO_ERR_MSG_LEN);

                    RewriteNode::interpolate_patched(
                        "
                        let __$query_subtype$s = match __$query_id$_matching_entities.get($idx$) {
                            Option::Some(raw_entities) => {
                                let mut raw_entities = *box::BoxTrait::unbox(raw_entities);
                                let mut entities: Array<$component$> = ArrayTrait::new();
                                loop {
                                    match raw_entities.pop_front() {
                                        Option::Some(raw) => {
                                            let mut raw = *raw;
                                            let e = serde::Serde::<$component$>::deserialize(ref \
                         raw).expect('$deser_err_msg$');
                                            entities.append(e);
                                        },
                                        Option::None(_) => {
                                            break ();
                                        }
                                    };
                                };
                                entities.span()
                            },
                            Option::None(_) => {
                                ArrayTrait::<$component$>::new().span()
                            }
                        };
                        ",
                        HashMap::from([
                            ("query_id".to_string(), RewriteNode::Text(command.query_id.clone())),
                            (
                                "query_subtype".to_string(),
                                RewriteNode::Text(component.to_string().to_ascii_lowercase()),
                            ),
                            ("idx".to_string(), RewriteNode::Text(idx.to_string())),
                            ("component".to_string(), RewriteNode::Text(component.to_string())),
                            ("deser_err_msg".to_string(), RewriteNode::Text(deser_err_msg)),
                        ]),
                    )
                })
                .collect::<Vec<_>>(),
        );

        let desered_entities: String = find_components(db, &command_ast)
            .iter()
            .map(|component| format!("__{}s", component.to_string().to_ascii_lowercase()))
            .collect::<Vec<String>>()
            .join(", ");

        command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
            let $query_id$ = ($desered_entities$);
            ",
            HashMap::from([
                ("query_id".to_string(), RewriteNode::Text(command.query_id.clone())),
                ("desered_entities".to_string(), RewriteNode::Text(desered_entities)),
            ]),
        ));

        command
    }

    fn rewrite_nodes(&self) -> Vec<RewriteNode> {
        self.data.rewrite_nodes.clone()
    }

    fn diagnostics(&self) -> Vec<PluginDiagnostic> {
        self.data.diagnostics.clone()
    }
}

pub fn find_components(db: &dyn SyntaxGroup, command_ast: &ast::ExprFunctionCall) -> Vec<SmolStr> {
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

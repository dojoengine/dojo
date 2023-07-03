use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_types::system::Dependency;
use sanitizer::StringSanitizer;
use smol_str::SmolStr;

use super::helpers::ast_arg_to_expr;
use super::{Command, CommandData, CommandMacroTrait, CAIRO_ERR_MSG_LEN};

pub struct FindCommand {
    query_id: String,
    data: CommandData,
    pub component_deps: Vec<Dependency>,
}

impl CommandMacroTrait for FindCommand {
    fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        macro_ast: ast::ExprInlineMacro,
    ) -> Self {
        let mut query_id =
            StringSanitizer::from(let_pattern.unwrap().as_syntax_node().get_text(db));
        query_id.to_snake_case();
        let mut command = FindCommand {
            query_id: query_id.get(),
            data: CommandData::new(),
            component_deps: vec![],
        };

        let elements = macro_ast.arguments(db).args(db).elements(db);

        if elements.len() != 3 {
            command.data.diagnostics.push(PluginDiagnostic {
                message: "Invalid arguments. Expected \"(world, query, (components,))\""
                    .to_string(),
                stable_ptr: macro_ast.arguments(db).as_syntax_node().stable_ptr(),
            });
            return command;
        }

        let world = &elements[0];
        let partition = &elements[1];
        let types = &elements[2];

        command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
            let mut __$query_id$_ids: Array<Span<felt252>> = ArrayTrait::new();
            let mut __$query_id$_entities_raw: Array<Span<Span<felt252>>> = ArrayTrait::new();

            ",
            UnorderedHashMap::from([(
                "query_id".to_string(),
                RewriteNode::Text(command.query_id.clone()),
            )]),
        ));

        let components = find_components(db, ast_arg_to_expr(db, types).unwrap());

        command.data.rewrite_nodes.extend(
            components
                .iter()
                .map(|component| {
                    RewriteNode::interpolate_patched(
                        "
                        let (__$query_id$_$query_subtype$_ids, __$query_id$_$query_subtype$_raw) = \
                         $world$.entities('$component$', $partition$);
                        __$query_id$_ids.append(__$query_id$_$query_subtype$_ids);
                        __$query_id$_entities_raw.append(__$query_id$_$query_subtype$_raw);
                        ",
                        UnorderedHashMap::from([
                            ("world".to_string(), RewriteNode::new_trimmed(world.as_syntax_node())),
                            ("query_id".to_string(), RewriteNode::Text(command.query_id.clone())),
                            (
                                "query_subtype".to_string(),
                                RewriteNode::Text(component.to_string().to_ascii_lowercase()),
                            ),
                            (
                                "partition".to_string(),
                                RewriteNode::new_trimmed(partition.as_syntax_node()),
                            ),
                            ("component".to_string(), RewriteNode::Text(component.to_string())),
                        ]),
                    )
                })
                .collect::<Vec<_>>(),
        );

        command.component_deps = components
            .iter()
            .map(|c| Dependency { name: c.to_string(), read: true, write: false })
            .collect();

        command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
            let mut __$query_id$_matching_entities = dojo::database::utils::find_matching(
                __$query_id$_ids.span(), __$query_id$_entities_raw.span()
            );

            ",
            UnorderedHashMap::from([(
                "query_id".to_string(),
                RewriteNode::Text(command.query_id.clone()),
            )]),
        ));

        command.data.rewrite_nodes.extend(
            components
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
                        UnorderedHashMap::from([
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

        let desered_entities: String = components
            .iter()
            .map(|component| format!("__{}s", component.to_string().to_ascii_lowercase()))
            .collect::<Vec<String>>()
            .join(", ");

        command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
            let $query_id$ = ($desered_entities$);
            ",
            UnorderedHashMap::from([
                ("query_id".to_string(), RewriteNode::Text(command.query_id.clone())),
                ("desered_entities".to_string(), RewriteNode::Text(desered_entities)),
            ]),
        ));

        command
    }
}

impl From<FindCommand> for Command {
    fn from(val: FindCommand) -> Self {
        Command::with_cmp_deps(val.data, val.component_deps)
    }
}

pub fn find_components(db: &dyn SyntaxGroup, expression: ast::Expr) -> Vec<SmolStr> {
    let mut components = vec![];
    match expression {
        ast::Expr::Tuple(tuple) => {
            for element in tuple.expressions(db).elements(db) {
                components.extend(find_components(db, element));
            }
        }
        ast::Expr::Parenthesized(parenthesized) => {
            components.extend(find_components(db, parenthesized.expr(db)));
        }
        ast::Expr::Path(path) => match path.elements(db).last().unwrap() {
            ast::PathSegment::WithGenericArgs(segment) => {
                let generic = segment.generic_args(db);

                for param in generic.generic_args(db).elements(db) {
                    if let ast::GenericArg::Expr(expr) = param {
                        components.extend(find_components(db, expr.value(db)));
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

use std::collections::HashMap;

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::ast::Arg;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use itertools::Itertools;
use sanitizer::StringSanitizer;
use smol_str::SmolStr;

use super::entities::find_components;
use super::{command_name, CommandData, CommandTrait};

pub struct EntityCommand {
    query_id: String,
    query_pattern: String,
    data: CommandData,
}

impl CommandTrait for EntityCommand {
    fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        command_ast: ast::ExprFunctionCall,
    ) -> Self {
        let command_name = command_name(db, command_ast.clone());
        let var_name = let_pattern.unwrap();
        let mut query_id = StringSanitizer::from(var_name.as_syntax_node().get_text(db));
        query_id.to_snake_case();
        let mut command = EntityCommand {
            query_id: query_id.get(),
            query_pattern: var_name.as_syntax_node().get_text(db),
            data: CommandData::new(),
        };

        let elements = command_ast.arguments(db).args(db).elements(db);
        let query = elements.first().unwrap();

        let components = find_components(db, command_ast);
        let part_names = components
            .iter()
            .map(|component| {
                format!(
                    "__{query_id}_{query_subtype}",
                    query_id = command.query_id,
                    query_subtype = component.to_string().to_ascii_lowercase()
                )
            })
            .collect();

        if command_name == "entity" {
            command.handle_entity(components, query, part_names);
        } else {
            command.handle_try_entity(components, query, part_names);
        }

        command
    }

    fn rewrite_nodes(&self) -> Vec<RewriteNode> {
        self.data.rewrite_nodes.clone()
    }

    fn diagnostics(&self) -> Vec<PluginDiagnostic> {
        self.data.diagnostics.clone()
    }
}

impl EntityCommand {
    fn handle_entity(&mut self, components: Vec<SmolStr>, query: &Arg, part_names: Vec<String>) {
        for component in components.iter() {
            self.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "
                    let mut __$query_id$_$query_subtype$_raw = IWorldDispatcher {
                        contract_address: world_address
                    }.entity('$component$', $query$, 0_u8, 0_usize);
                    assert(__$query_id$_$query_subtype$_raw.len() > 0_usize, 'Failed to find \
                 $component$');
                    let __$query_id$_$query_subtype$ = serde::Serde::<$component$>::deserialize(
                        ref __$query_id$_$query_subtype$_raw
                    ).expect('Failed to deserialize $component$');
                    ",
                HashMap::from([
                    ("component".to_string(), RewriteNode::Text(component.to_string())),
                    (
                        "query_subtype".to_string(),
                        RewriteNode::Text(component.to_string().to_ascii_lowercase()),
                    ),
                    ("query_id".to_string(), RewriteNode::Text(self.query_id.clone())),
                    ("query".to_string(), RewriteNode::new_trimmed(query.as_syntax_node())),
                ]),
            ));
        }

        let part_names_str = if components.len() > 1 {
            format!("({})", part_names.join(", "))
        } else {
            part_names.join(", ")
        };

        self.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "let $query_pattern$ = $part_names$;
                    ",
            HashMap::from([
                ("query_pattern".to_string(), RewriteNode::Text(self.query_pattern.clone())),
                ("part_names".to_string(), RewriteNode::Text(part_names_str)),
            ]),
        ));
    }

    fn handle_try_entity(
        &mut self,
        components: Vec<SmolStr>,
        query: &Arg,
        part_names: Vec<String>,
    ) {
        for component in components.iter() {
            self.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "
                    let mut __$query_id$_$query_subtype$_raw = IWorldDispatcher {
                        contract_address: world_address
                    }.entity('$component$', $query$, 0_u8, 0_usize);
                    let __$query_id$_$query_subtype$ = match \
                 __$query_id$_$query_subtype$_raw.len() > 0_usize {
                        bool::False(()) => {
                            Option::None(())
                        },
                        bool::True(()) => {
                            Option::Some(serde::Serde::<$component$>::deserialize(
                                ref __$query_id$_$query_subtype$_raw
                            ).expect('Failed to deserialize $component$'))
                        }
                    };
                    ",
                HashMap::from([
                    ("component".to_string(), RewriteNode::Text(component.to_string())),
                    (
                        "query_subtype".to_string(),
                        RewriteNode::Text(component.to_string().to_ascii_lowercase()),
                    ),
                    ("query_id".to_string(), RewriteNode::Text(self.query_id.clone())),
                    ("query".to_string(), RewriteNode::new_trimmed(query.as_syntax_node())),
                ]),
            ));
        }

        let part_names_condition_str =
            part_names.iter().map(|part_name| format!("{part_name}.is_some()")).join(" & ");

        let part_names_str = if components.len() > 1 {
            format!("({})", part_names.join(", "))
        } else {
            part_names.join(", ")
        };

        self.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "let $query_pattern$ = if $part_names_condition${
                    Option::Some($part_names$)
                } else {
                    Option::None(())
                };
            ",
            HashMap::from([
                ("query_pattern".to_string(), RewriteNode::Text(self.query_pattern.clone())),
                ("part_names_condition".to_string(), RewriteNode::Text(part_names_condition_str)),
                ("part_names".to_string(), RewriteNode::Text(part_names_str)),
            ]),
        ));
    }
}

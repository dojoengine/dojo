use std::collections::HashMap;

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use itertools::Itertools;
use sanitizer::StringSanitizer;

use super::all::find_components;
use super::{CommandData, CommandTrait};

pub struct GetCommand {
    query_id: String,
    query_pattern: String,
    data: CommandData,
}

impl CommandTrait for GetCommand {
    fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        command_ast: ast::ExprFunctionCall,
    ) -> Self {
        let var_name = let_pattern.unwrap();
        let mut query_id = StringSanitizer::from(var_name.as_syntax_node().get_text(db));
        query_id.to_snake_case();
        let mut command = GetCommand {
            query_id: query_id.get(),
            query_pattern: var_name.as_syntax_node().get_text(db),
            data: CommandData::new(),
        };

        let elements = command_ast.arguments(db).args(db).elements(db);
        let storage_key = elements.first().unwrap();

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
            .join(", ");

        for component in components.iter() {
            command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "
                    let mut __$query_id$_$query_subtype$_raw = IWorldDispatcher {
                        contract_address: world_address
                    }.get('$component$', $storage_key$, 0_u8, 0_usize);
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
                    ("query_id".to_string(), RewriteNode::Text(command.query_id.clone())),
                    (
                        "storage_key".to_string(),
                        RewriteNode::new_trimmed(storage_key.as_syntax_node()),
                    ),
                ]),
            ));
        }

        if components.len() > 1 {
            command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "let $query_pattern$ = ($part_names$);
                    ",
                HashMap::from([
                    ("query_pattern".to_string(), RewriteNode::Text(command.query_pattern.clone())),
                    ("part_names".to_string(), RewriteNode::Text(part_names)),
                ]),
            ));
        } else {
            command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "let $query_pattern$ = $part_names$;
                    ",
                HashMap::from([
                    ("query_pattern".to_string(), RewriteNode::Text(command.query_pattern.clone())),
                    ("part_names".to_string(), RewriteNode::Text(part_names)),
                ]),
            ));
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

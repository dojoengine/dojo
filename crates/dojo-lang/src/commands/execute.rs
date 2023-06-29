use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;

use super::helpers::context_arg_as_path_segment_simple_or_panic;
use super::{Command, CommandData, CommandMacroTrait};

pub struct ExecuteCommand {
    data: CommandData,
}

impl CommandMacroTrait for ExecuteCommand {
    fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        macro_ast: ast::ExprInlineMacro,
    ) -> Self {
        let mut command = ExecuteCommand { data: CommandData::new() };

        let elements = macro_ast.arguments(db).args(db).elements(db);

        if elements.len() != 4 {
            command.data.diagnostics.push(PluginDiagnostic {
                message: "Invalid arguments. Expected \"(world, system, query, calldata)\""
                    .to_string(),
                stable_ptr: macro_ast.arguments(db).as_syntax_node().stable_ptr(),
            });
            return command;
        }

        let context = &elements[0];
        let system = &elements[1];
        let calldata = &elements[3];

        let context_name =
            context_arg_as_path_segment_simple_or_panic(db, context).ident(db).text(db);

        if let Some(var_name) = let_pattern {
            command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "let $var_name$ = $context$.world.execute('$system$', $calldata$);
                ",
                UnorderedHashMap::from([
                    ("context".to_string(), RewriteNode::Text(context_name.to_string())),
                    ("var_name".to_string(), RewriteNode::new_trimmed(var_name.as_syntax_node())),
                    ("system".to_string(), RewriteNode::new_trimmed(system.as_syntax_node())),
                    ("calldata".to_string(), RewriteNode::new_trimmed(calldata.as_syntax_node())),
                ]),
            ));
        } else {
            command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "$context$.world.execute('$system$', $calldata$);
                ",
                UnorderedHashMap::from([
                    ("context".to_string(), RewriteNode::Text(context_name.to_string())),
                    ("system".to_string(), RewriteNode::new_trimmed(system.as_syntax_node())),
                    ("calldata".to_string(), RewriteNode::new_trimmed(calldata.as_syntax_node())),
                ]),
            ));
        }

        command
    }
}

impl From<ExecuteCommand> for Command {
    fn from(value: ExecuteCommand) -> Self {
        Command::with_data(value.data)
    }
}

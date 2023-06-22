use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;

use super::{Command, CommandData, CommandMacroTrait};

pub struct UUIDCommand {
    data: CommandData,
}

impl CommandMacroTrait for UUIDCommand {
    fn from_ast(
        _db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        _command_ast: ast::ExprInlineMacro,
    ) -> Self {
        let mut command = UUIDCommand { data: CommandData::new() };

        command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "let $var_name$ = ctx.world.uuid();
                    ",
            UnorderedHashMap::from([(
                "var_name".to_string(),
                RewriteNode::new_trimmed(let_pattern.unwrap().as_syntax_node()),
            )]),
        ));

        command
    }
}

impl Into<Command> for UUIDCommand {
    fn into(self) -> Command {
        Command::with_data(self.data)
    }
}

// spawn_command.rs
use std::collections::HashMap;

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};

use super::{CommandData, CommandTrait};

pub struct UUIDCommand {
    data: CommandData,
}

impl CommandTrait for UUIDCommand {
    fn from_ast(
        _db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        _command_ast: ast::ExprFunctionCall,
    ) -> Self {
        let mut command = UUIDCommand { data: CommandData::new() };

        command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "let $var_name$ = ctx.world.uuid();
                    ",
            HashMap::from([(
                "var_name".to_string(),
                RewriteNode::new_trimmed(let_pattern.unwrap().as_syntax_node()),
            )]),
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

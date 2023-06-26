use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;

use super::{CommandData, CommandTrait};

pub struct ExecuteCommand {
    data: CommandData,
}

impl CommandTrait for ExecuteCommand {
    fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        command_ast: ast::ExprFunctionCall,
    ) -> Self {
        let mut command = ExecuteCommand { data: CommandData::new() };

        let elements = command_ast.arguments(db).args(db).elements(db);
        let system = elements.first().unwrap();
        let calldata = elements.last().unwrap();

        if let Some(var_name) = let_pattern {
            command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "let $var_name$ = ctx.world.execute('$system$', $calldata$);
                ",
                UnorderedHashMap::from([
                    ("var_name".to_string(), RewriteNode::new_trimmed(var_name.as_syntax_node())),
                    ("system".to_string(), RewriteNode::new_trimmed(system.as_syntax_node())),
                    ("calldata".to_string(), RewriteNode::new_trimmed(calldata.as_syntax_node())),
                ]),
            ));
        } else {
            command.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "ctx.world.execute('$system$', $calldata$);
                ",
                UnorderedHashMap::from([
                    ("system".to_string(), RewriteNode::new_trimmed(system.as_syntax_node())),
                    ("calldata".to_string(), RewriteNode::new_trimmed(calldata.as_syntax_node())),
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

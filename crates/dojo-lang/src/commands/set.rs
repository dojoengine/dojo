use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_types::system::Dependency;

use super::{Command, CommandData, CommandMacroTrait};

#[derive(Clone)]
pub struct SetCommand {
    data: CommandData,
    component_deps: Vec<Dependency>,
}

impl SetCommand {
    fn handle_struct(
        &mut self,
        db: &dyn SyntaxGroup,
        world: &ast::Arg,
        query: ast::Arg,
        expr: ast::Expr,
    ) {
        if let ast::Expr::StructCtorCall(ctor) = expr {
            if let Some(ast::PathSegment::Simple(segment)) = ctor.path(db).elements(db).last() {
                let component_name = segment.ident(db).text(db);

                self.component_deps.push(Dependency {
                    name: component_name.to_string(),
                    write: true,
                    read: false,
                });
                self.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
                    "
                    {
                        let mut calldata = array::ArrayTrait::new();
                        serde::Serde::serialize(@$ctor$, ref calldata);
                        $world$.set_entity('$component$', $query$, 0_u8, \
                     array::ArrayTrait::span(@calldata));
                    }
                    ",
                    UnorderedHashMap::from([
                        ("world".to_string(), RewriteNode::new_trimmed(world.as_syntax_node())),
                        ("component".to_string(), RewriteNode::Text(component_name.to_string())),
                        ("ctor".to_string(), RewriteNode::new_trimmed(ctor.as_syntax_node())),
                        ("query".to_string(), RewriteNode::new_trimmed(query.as_syntax_node())),
                    ]),
                ));
            }
        }
    }
}

impl CommandMacroTrait for SetCommand {
    fn from_ast(
        db: &dyn SyntaxGroup,
        _let_pattern: Option<ast::Pattern>,
        command_ast: ast::ExprInlineMacro,
    ) -> Self {
        let mut command = SetCommand { data: CommandData::new(), component_deps: vec![] };

        let elements = command_ast.arguments(db).args(db).elements(db);

        if elements.len() != 3 {
            command.data.diagnostics.push(PluginDiagnostic {
                message: "Invalid arguments. Expected \"(world, query, (components,))\""
                    .to_string(),
                stable_ptr: command_ast.arguments(db).as_syntax_node().stable_ptr(),
            });
            return command;
        }

        let world = &elements[0];
        let query = elements[1].clone();
        let bundle = &elements[2];
        if let ast::ArgClause::Unnamed(clause) = bundle.arg_clause(db) {
            match clause.value(db) {
                ast::Expr::Parenthesized(bundle) => {
                    command.handle_struct(db, world, query, bundle.expr(db));
                }
                ast::Expr::Tuple(tuple) => {
                    for expr in tuple.expressions(db).elements(db) {
                        command.handle_struct(db, world, query.clone(), expr);
                    }
                }
                _ => {
                    command.data.diagnostics.push(PluginDiagnostic {
                        message: "Invalid storage key. Expected \"(...)\"".to_string(),
                        stable_ptr: clause.as_syntax_node().stable_ptr(),
                    });
                }
            }
        }

        command
    }
}

impl From<SetCommand> for Command {
    fn from(val: SetCommand) -> Self {
        Command::with_cmp_deps(val.data, val.component_deps)
    }
}

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
        world: &ast::Expr,
        query: &ast::Expr,
        expr: &ast::Expr,
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
        macro_ast: ast::ExprInlineMacro,
    ) -> Self {
        let mut command = SetCommand { data: CommandData::new(), component_deps: vec![] };

        let wrapped_args = macro_ast.arguments(db);
        let exprs = match wrapped_args {
            ast::WrappedExprList::ParenthesizedExprList(args_list) => {
                args_list.expressions(db).elements(db)
            }
            _ => {
                command.data.diagnostics.push(PluginDiagnostic {
                    message: "Invalid macro. Expected \"set!(world, query, (components,))\""
                        .to_string(),
                    stable_ptr: macro_ast.arguments(db).as_syntax_node().stable_ptr(),
                });
                return command;
            }
        };

        if exprs.len() != 3 {
            command.data.diagnostics.push(PluginDiagnostic {
                message: "Invalid arguments. Expected \"(world, query, (components,))\""
                    .to_string(),
                stable_ptr: macro_ast.arguments(db).as_syntax_node().stable_ptr(),
            });
            return command;
        }

        let world = &exprs[0];
        let query = &exprs[1].clone();
        let bundle = &exprs[2];
        command.handle_struct(db, world, query, bundle);

        command
    }
}

impl From<SetCommand> for Command {
    fn from(val: SetCommand) -> Self {
        Command::with_cmp_deps(val.data, val.component_deps)
    }
}

use crate::inline_macro_plugin::{InlineMacro, InlineMacroExpanderData};

pub struct SetMacro;
impl InlineMacro for SetMacro {
    fn append_macro_code(
        &self,
        macro_expander_data: &mut InlineMacroExpanderData,
        db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
        macro_arguments: &cairo_lang_syntax::node::ast::ExprList,
    ) {
        let args = macro_arguments.elements(db);

        if args.len() != 3 {
            macro_expander_data.diagnostics.push(PluginDiagnostic {
                message: "Invalid arguments. Expected \"(world, query, (components,))\""
                    .to_string(),
                stable_ptr: macro_arguments.as_syntax_node().stable_ptr(),
            });
            return;
        }

        let world = &args[0];
        let query = &args[1].clone();
        let bundle = &args[2];

        let mut expanded_code = "{
            let mut __array_builder_macro_result__ = ArrayTrait::new();"
            .to_string();
        for arg in args {
            expanded_code.push_str(&format!(
                "\n            array::ArrayTrait::append(ref __array_builder_macro_result__, {});",
                arg.as_syntax_node().get_text(db)
            ));
        }
        expanded_code.push_str(
            "\n            __array_builder_macro_result__
        }",
        );
        macro_expander_data.result_code.push_str(&expanded_code);
        macro_expander_data.code_changed = true;
    }

    fn is_bracket_type_allowed(
        &self,
        db: &dyn cairo_lang_syntax::node::db::SyntaxGroup,
        macro_ast: &cairo_lang_syntax::node::ast::ExprInlineMacro,
    ) -> bool {
        matches!(
            macro_ast.arguments(db),
            cairo_lang_syntax::node::ast::WrappedExprList::ParenthesizedExprList(_)
        )
    }
}

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::ast::Arg;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_types::system::Dependency;
use itertools::Itertools;
use sanitizer::StringSanitizer;
use smol_str::SmolStr;

use super::find::find_components;
use super::helpers::{ast_arg_to_expr, macro_name};
use super::{Command, CommandData, CommandMacroTrait, CAIRO_ERR_MSG_LEN};

pub struct GetCommand {
    query_id: String,
    query_pattern: String,
    data: CommandData,
    component_deps: Vec<Dependency>,
}

impl CommandMacroTrait for GetCommand {
    fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        macro_ast: ast::ExprInlineMacro,
    ) -> Self {
        let macro_name = macro_name(db, macro_ast.clone());
        let var_name = let_pattern.unwrap();
        let mut query_id = StringSanitizer::from(var_name.as_syntax_node().get_text(db));
        query_id.to_snake_case();
        let mut command = GetCommand {
            query_id: query_id.get(),
            query_pattern: var_name.as_syntax_node().get_text(db),
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
        let query = &elements[1];
        let types = &elements[2];

        let components = find_components(db, ast_arg_to_expr(db, types).unwrap());

        if components.is_empty() {
            command.data.diagnostics.push(PluginDiagnostic {
                message: "Component types cannot be empty".to_string(),
                stable_ptr: macro_ast.stable_ptr().untyped(),
            });
            return command;
        }

        command.component_deps = components
            .iter()
            .map(|c| Dependency { name: c.to_string(), read: true, write: false })
            .collect();

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

        if macro_name == "get" {
            command.handle_get(components, world, query, part_names);
        } else {
            command.handle_try_get(components, world, query, part_names);
        }

        command
    }
}

impl From<GetCommand> for Command {
    fn from(val: GetCommand) -> Self {
        Command::with_cmp_deps(val.data, val.component_deps)
    }
}

impl GetCommand {
    fn handle_get(
        &mut self,
        components: Vec<SmolStr>,
        world: &Arg,
        query: &Arg,
        part_names: Vec<String>,
    ) {
        for component in components.iter() {
            let mut lookup_err_msg = format!("{} not found", component.to_string());
            lookup_err_msg.truncate(CAIRO_ERR_MSG_LEN);
            let mut deser_err_msg = format!("{} failed to deserialize", component.to_string());
            deser_err_msg.truncate(CAIRO_ERR_MSG_LEN);

            self.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "
                    let mut __$query_id$_$query_subtype$_raw = $world$.entity('$component$', \
                 $query$, 0_u8, 0_usize);
                    assert(__$query_id$_$query_subtype$_raw.len() > 0_usize, '$lookup_err_msg$');
                    let __$query_id$_$query_subtype$ = serde::Serde::<$component$>::deserialize(
                        ref __$query_id$_$query_subtype$_raw
                    ).expect('$deser_err_msg$');
                    ",
                UnorderedHashMap::from([
                    ("world".to_string(), RewriteNode::new_trimmed(world.as_syntax_node())),
                    ("component".to_string(), RewriteNode::Text(component.to_string())),
                    (
                        "query_subtype".to_string(),
                        RewriteNode::Text(component.to_string().to_ascii_lowercase()),
                    ),
                    ("query_id".to_string(), RewriteNode::Text(self.query_id.clone())),
                    ("query".to_string(), RewriteNode::new_trimmed(query.as_syntax_node())),
                    ("lookup_err_msg".to_string(), RewriteNode::Text(lookup_err_msg)),
                    ("deser_err_msg".to_string(), RewriteNode::Text(deser_err_msg)),
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
            UnorderedHashMap::from([
                ("query_pattern".to_string(), RewriteNode::Text(self.query_pattern.clone())),
                ("part_names".to_string(), RewriteNode::Text(part_names_str)),
            ]),
        ));
    }

    fn handle_try_get(
        &mut self,
        components: Vec<SmolStr>,
        world: &Arg,
        query: &Arg,
        part_names: Vec<String>,
    ) {
        for component in components.iter() {
            let mut deser_err_msg = format!("{} failed to deserialize", component.to_string());
            deser_err_msg.truncate(CAIRO_ERR_MSG_LEN);

            self.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
                "
                    let mut __$query_id$_$query_subtype$_raw = $world$.entity('$component$', \
                 $query$, 0_u8, 0_usize);
                    let __$query_id$_$query_subtype$ = match \
                 __$query_id$_$query_subtype$_raw.len() > 0_usize {
                        bool::False(()) => {
                            Option::None(())
                        },
                        bool::True(()) => {
                            Option::Some(serde::Serde::<$component$>::deserialize(
                                ref __$query_id$_$query_subtype$_raw
                            ).expect('$deser_err_msg$'))
                        }
                    };
                    ",
                UnorderedHashMap::from([
                    ("world".to_string(), RewriteNode::new_trimmed(world.as_syntax_node())),
                    ("component".to_string(), RewriteNode::Text(component.to_string())),
                    (
                        "query_subtype".to_string(),
                        RewriteNode::Text(component.to_string().to_ascii_lowercase()),
                    ),
                    ("query_id".to_string(), RewriteNode::Text(self.query_id.clone())),
                    ("query".to_string(), RewriteNode::new_trimmed(query.as_syntax_node())),
                    ("deser_err_msg".to_string(), RewriteNode::Text(deser_err_msg)),
                ]),
            ));
        }

        let part_names_condition_str =
            part_names.iter().map(|part_name| format!("{part_name}.is_some()")).join(" & ");

        let part_names_str = match part_names.len() {
            1 => format!("{}.unwrap()", part_names[0]),
            _ => format!("({}.unwrap())", part_names.join(".unwrap(), ")),
        };

        self.data.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "let $query_pattern$ = if $part_names_condition${
                    Option::Some($part_names$)
                } else {
                    Option::None(())
                };
            ",
            UnorderedHashMap::from([
                ("query_pattern".to_string(), RewriteNode::Text(self.query_pattern.clone())),
                ("part_names_condition".to_string(), RewriteNode::Text(part_names_condition_str)),
                ("part_names".to_string(), RewriteNode::Text(part_names_str)),
            ]),
        ));
    }
}

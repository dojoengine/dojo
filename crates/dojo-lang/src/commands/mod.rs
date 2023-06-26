use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal};
use dojo_world::manifest::Dependency;

use self::entities::EntitiesCommand;
use self::entity::EntityCommand;
use self::execute::ExecuteCommand;
use self::set::SetCommand;
use self::uuid::UUIDCommand;

pub mod entities;
pub mod entity;
pub mod execute;
mod helpers;
pub mod set;
pub mod uuid;

const CAIRO_ERR_MSG_LEN: usize = 31;

pub trait CommandMacroTrait: Into<Command> {
    fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        macro_ast: ast::ExprInlineMacro,
    ) -> Self;
}

#[derive(Clone)]
pub struct CommandData {
    rewrite_nodes: Vec<RewriteNode>,
    diagnostics: Vec<PluginDiagnostic>,
}

impl CommandData {
    pub fn new() -> Self {
        CommandData { rewrite_nodes: vec![], diagnostics: vec![] }
    }
}
pub struct Command {
    pub rewrite_nodes: Vec<RewriteNode>,
    pub diagnostics: Vec<PluginDiagnostic>,
    pub component_deps: Vec<Dependency>,
}

impl Command {
    pub fn with_data(data: CommandData) -> Self {
        Command {
            rewrite_nodes: data.rewrite_nodes,
            diagnostics: data.diagnostics,
            component_deps: vec![],
        }
    }

    /// With component dependencies
    pub fn with_cmp_deps(data: CommandData, component_deps: Vec<Dependency>) -> Self {
        Command { rewrite_nodes: data.rewrite_nodes, diagnostics: data.diagnostics, component_deps }
    }

    pub fn try_from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        macro_ast: ast::ExprInlineMacro,
    ) -> Option<Self> {
        let elements = macro_ast.path(db).elements(db);
        let segment = elements.last().unwrap();
        match segment {
            ast::PathSegment::Simple(segment_simple) => {
                match segment_simple.ident(db).text(db).as_str() {
                    "uuid" => Some(UUIDCommand::from_ast(db, let_pattern, macro_ast).into()),
                    "set" => Some(SetCommand::from_ast(db, let_pattern, macro_ast).into()),
                    "entity" | "try_entity" => {
                        Some(EntityCommand::from_ast(db, let_pattern, macro_ast).into())
                    }
                    "entities" => {
                        Some(EntitiesCommand::from_ast(db, let_pattern, macro_ast).into())
                    }
                    "execute" => Some(ExecuteCommand::from_ast(db, let_pattern, macro_ast).into()),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

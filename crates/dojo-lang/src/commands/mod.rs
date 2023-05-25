use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal};
use smol_str::SmolStr;

pub mod entities;
pub mod entity;
pub mod execute;
pub mod set;
pub mod uuid;

const CAIRO_ERR_MSG_LEN: usize = 31;

pub trait CommandTrait {
    fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        command_ast: ast::ExprFunctionCall,
    ) -> Self;

    fn rewrite_nodes(&self) -> Vec<RewriteNode>;
    fn diagnostics(&self) -> Vec<PluginDiagnostic>;
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
    pub component_deps: Vec<SmolStr>,
}

impl Command {
    pub fn from_ast(
        db: &dyn SyntaxGroup,
        let_pattern: Option<ast::Pattern>,
        command_ast: ast::ExprFunctionCall,
    ) -> Self {
        let mut command =
            Command { rewrite_nodes: vec![], diagnostics: vec![], component_deps: vec![] };

        match command_name(db, command_ast.clone()).as_str() {
            "uuid" => {
                let sc = uuid::UUIDCommand::from_ast(db, let_pattern, command_ast);
                command.rewrite_nodes.extend(sc.rewrite_nodes());
                command.diagnostics.extend(sc.diagnostics());
            }
            "entity" => {
                let sc = entity::EntityCommand::from_ast(db, let_pattern, command_ast);
                command.rewrite_nodes.extend(sc.rewrite_nodes());
                command.diagnostics.extend(sc.diagnostics());
            }
            "try_entity" => {
                let sc = entity::EntityCommand::from_ast(db, let_pattern, command_ast);
                command.rewrite_nodes.extend(sc.rewrite_nodes());
                command.diagnostics.extend(sc.diagnostics());
            }
            "set_entity" => {
                let sc = set::SetCommand::from_ast(db, let_pattern, command_ast);
                command.rewrite_nodes.extend(sc.rewrite_nodes());
                command.diagnostics.extend(sc.diagnostics());
                command.component_deps.extend(sc.components);
            }
            "entities" => {
                let sc = entities::EntitiesCommand::from_ast(db, let_pattern, command_ast);
                command.rewrite_nodes.extend(sc.rewrite_nodes());
                command.diagnostics.extend(sc.diagnostics());
            }
            "execute" => {
                let sc = execute::ExecuteCommand::from_ast(db, let_pattern, command_ast);
                command.rewrite_nodes.extend(sc.rewrite_nodes());
                command.diagnostics.extend(sc.diagnostics());
            }
            _ => {}
        }

        command
    }
}

pub fn command_name(db: &dyn SyntaxGroup, command_ast: ast::ExprFunctionCall) -> SmolStr {
    let elements = command_ast.path(db).elements(db);
    let segment = elements.last().unwrap();
    if let ast::PathSegment::Simple(method) = segment {
        method.ident(db).text(db)
    } else if let ast::PathSegment::WithGenericArgs(generic) = segment {
        generic.ident(db).text(db)
    } else {
        SmolStr::new("")
    }
}

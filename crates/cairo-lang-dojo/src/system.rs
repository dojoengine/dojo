use std::collections::HashMap;

use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, PluginDiagnostic, PluginGeneratedFile, PluginResult,
};
use cairo_lang_semantic::patcher::{ModifiedNode, PatchBuilder, RewriteNode};
use cairo_lang_semantic::plugin::DynPluginAuxData;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use cairo_lang_utils::try_extract_matches;
use indoc::formatdoc;
use smol_str::SmolStr;

use crate::plugin::DojoAuxData;
use crate::query::Query;

pub struct System {
    pub name: SmolStr,
    pub rewrite_nodes: Vec<RewriteNode>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

impl System {
    pub fn from_module_body(db: &dyn SyntaxGroup, name: SmolStr, body: ast::ModuleBody) -> Self {
        let diagnostics = vec![];
        let rewrite_nodes: Vec<RewriteNode> = vec![];
        let mut system = System { rewrite_nodes, name, diagnostics };

        let mut matched_execute = false;
        for item in body.items(db).elements(db) {
            match &item {
                ast::Item::FreeFunction(item_function) => {
                    let name = item_function.declaration(db).name(db).text(db);
                    if name == "execute" && matched_execute {
                        system.diagnostics.push(PluginDiagnostic {
                            message: "Only one execute function per module is supported."
                                .to_string(),
                            stable_ptr: item_function.stable_ptr().untyped(),
                        });
                        continue;
                    }

                    if name == "execute" {
                        system.handle_function(db, item_function.clone());
                        matched_execute = true;
                        continue;
                    }

                    system.rewrite_nodes.push(RewriteNode::Copied(item_function.as_syntax_node()))
                }
                item => system.rewrite_nodes.push(RewriteNode::Copied(item.as_syntax_node())),
            }
        }

        system
    }

    pub fn result(self, db: &dyn SyntaxGroup) -> PluginResult {
        let name = self.name;
        let mut builder = PatchBuilder::new(db);
        builder.add_modified(RewriteNode::interpolate_patched(
            &formatdoc!(
                "
                #[contract]
                mod {name} {{
                    $body$
                }}
                ",
            ),
            HashMap::from([(
                "body".to_string(),
                RewriteNode::Modified(ModifiedNode { children: self.rewrite_nodes }),
            )]),
        ));

        PluginResult {
            code: Some(PluginGeneratedFile {
                name,
                content: builder.code,
                aux_data: DynGeneratedFileAuxData::new(DynPluginAuxData::new(DojoAuxData {
                    patches: builder.patches,
                })),
            }),
            diagnostics: self.diagnostics,
            remove_original_item: true,
        }
    }

    fn handle_function(&mut self, db: &dyn SyntaxGroup, function_ast: ast::FunctionWithBody) {
        let signature = function_ast.declaration(db).signature(db);
        let parameters = signature.parameters(db).elements(db);
        let mut preprocess_rewrite_nodes = vec![];

        for param in parameters.iter() {
            let type_ast = param.type_clause(db).ty(db);

            if let Some(SystemArgType::Query) = try_extract_execute_paramters(db, &type_ast) {
                let query = Query::from_expr(db, type_ast.clone());
                preprocess_rewrite_nodes.extend(query.rewrite_nodes);
            }
        }

        let name = self.name.clone();
        self.rewrite_nodes.push(RewriteNode::interpolate_patched(
            &formatdoc!(
                "
                struct Storage {{
                    world_address: felt,
                }}
    
                #[external]
                fn initialize(world_addr: felt) {{
                    let world = world_address::read();
                    assert(world == 0, '{name}: Already initialized.');
                    world_address::write(world_addr);
                }}
    
                #[external]
                fn execute() {{
                    let world = world_address::read();
                    assert(world != 0, '{name}: Not initialized.');
    
                    $preprocessing$
    
                    $body$
                }}
                "
            ),
            HashMap::from([
                (
                    "body".to_string(),
                    RewriteNode::Trimmed(function_ast.body(db).statements(db).as_syntax_node()),
                ),
                (
                    "preprocessing".to_string(),
                    RewriteNode::Modified(ModifiedNode { children: preprocess_rewrite_nodes }),
                ),
            ]),
        ));
    }
}

enum SystemArgType {
    Query,
}

fn try_extract_execute_paramters(
    db: &dyn SyntaxGroup,
    type_ast: &ast::Expr,
) -> Option<SystemArgType> {
    let as_path = try_extract_matches!(type_ast, ast::Expr::Path)?;
    let binding = as_path.elements(db);
    let last = binding.last()?;
    let segment = match last {
        ast::PathSegment::WithGenericArgs(segment) => segment,
        ast::PathSegment::Simple(_segment) => {
            // TODO: Match `world` var name.
            return None;
        }
    };
    let ty = segment.ident(db).text(db);

    if ty == "Query" {
        Some(SystemArgType::Query)
    } else {
        None
    }
}

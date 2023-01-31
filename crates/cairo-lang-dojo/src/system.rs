use std::collections::HashMap;

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_starknet::contract::starknet_keccak;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use indoc::formatdoc;
use itertools::Itertools;

pub struct System {
    pub rewrite_nodes: Vec<RewriteNode>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

impl System {
    pub fn from_module_body(db: &dyn SyntaxGroup, body: ast::ModuleBody) -> Self {
        let diagnostics = vec![];
        let rewrite_nodes: Vec<RewriteNode> = vec![];
        let mut system = System { rewrite_nodes, diagnostics };

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
                        system.handle_system_function(db, item_function.clone());
                        matched_execute = true;
                    }
                }
                _ => (),
            }
        }

        system
    }

    fn handle_system_function(
        &mut self,
        db: &dyn SyntaxGroup,
        function_ast: ast::FunctionWithBody,
    ) {
        let name = function_ast.declaration(db).name(db).text(db);
        let system_name = format!("{}System", name[0..1].to_uppercase() + &name[1..]);
        let signature = function_ast.declaration(db).signature(db);
        let parameters = signature.parameters(db).elements(db);

        let query_param = parameters
            .iter()
            .find(|attr| match attr.name(db) {
                name => name.text(db).as_str() == "query",
            })
            .unwrap();

        let generic_types;
        match query_param.type_clause(db).ty(db) {
            ast::Expr::Path(path) => {
                let generic = path
                    .elements(db)
                    .iter()
                    .find_map(|segment| match segment {
                        ast::PathSegment::WithGenericArgs(segment) => {
                            if segment.ident(db).text(db).as_str() == "Query" {
                                Some(segment.generic_args(db))
                            } else {
                                None
                            }
                        }
                        _ => None,
                    })
                    .unwrap();

                generic_types = generic.generic_args(db).elements(db);
            }
            _ => return,
        }

        let query_lookup = generic_types
            .iter()
            .map(|f| {
                format!(
                    "let {} = IWorld.lookup(world, {:#x});",
                    f.as_syntax_node().get_text(db).to_ascii_lowercase() + "_ids",
                    starknet_keccak(f.as_syntax_node().get_text(db).as_bytes())
                )
            })
            .join("\n");

        self.rewrite_nodes.push(RewriteNode::interpolate_patched(
            &formatdoc!(
                "
                struct Storage {{
                    world_address: felt,
                }}
    
                #[external]
                fn initialize(world_addr: felt) {{
                    let world = world_address::read();
                    assert(world == 0, '{system_name}: Already initialized.');
                    world_address::write(world_addr);
                }}
    
                #[external]
                fn execute() {{
                    let world = world_address::read();
                    assert(world != 0, '{system_name}: Not initialized.');
    
                    {query_lookup}
    
                    $body$
                }}
                "
            ),
            HashMap::from([
                (
                    "type_name".to_string(),
                    RewriteNode::Trimmed(function_ast.declaration(db).name(db).as_syntax_node()),
                ),
                (
                    "body".to_string(),
                    RewriteNode::Trimmed(function_ast.body(db).statements(db).as_syntax_node()),
                ),
                ("query_param".to_string(), RewriteNode::Trimmed(query_param.as_syntax_node())),
                // ("parameters".to_string(), RewriteNode::Trimmed(function_ast.declaration(db).signature(db).parameters(db).as_syntax_node())),
            ]),
        ));
    }
}

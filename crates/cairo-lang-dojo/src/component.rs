use std::collections::HashMap;

use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_semantic::patcher::RewriteNode;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, TypedSyntaxNode};

pub struct Component {
    pub rewrite_nodes: Vec<RewriteNode>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

impl Component {
    pub fn from_module_body(db: &dyn SyntaxGroup, body: ast::ModuleBody) -> Self {
        let diagnostics = vec![];
        let rewrite_nodes: Vec<RewriteNode> = vec![];
        let mut component = Component { rewrite_nodes, diagnostics };

        let mut matched_struct = false;
        for item in body.items(db).elements(db) {
            match &item {
                ast::Item::Struct(item_struct) => {
                    if matched_struct {
                        component.diagnostics.push(PluginDiagnostic {
                            message: "Only one struct per module is supported.".to_string(),
                            stable_ptr: item_struct.stable_ptr().untyped(),
                        });
                        continue;
                    }

                    component.handle_component_struct(db, item_struct.clone());
                    matched_struct = true;
                }
                ast::Item::FreeFunction(item_function) => {
                    component.handle_component_functions(db, item_function.clone());
                }
                _ => (),
            }
        }

        component
    }

    fn handle_component_struct(&mut self, db: &dyn SyntaxGroup, struct_ast: ast::ItemStruct) {
        self.rewrite_nodes.push(RewriteNode::Copied(struct_ast.as_syntax_node()));
        self.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
                    struct Storage {
                        world_address: felt,
                        state: Map::<felt, $type_name$>,
                    }
    
                    // Initialize $type_name$Component.
                    #[external]
                    fn initialize(world_addr: felt) {
                        let world = world_address::read();
                        assert(world == 0, '$type_name$Component: Already initialized.');
                        world_address::write(world_addr);
                    }
    
                    // Set the state of an entity.
                    #[external]
                    fn set(entity_id: felt, value: $type_name$) {
                        state::write(entity_id, value);
                    }
    
                    // Get the state of an entity.
                    #[view]
                    fn get(entity_id: felt) -> $type_name$ {
                        return state::read(entity_id);
                    }
                    ",
            HashMap::from([(
                "type_name".to_string(),
                RewriteNode::Trimmed(struct_ast.name(db).as_syntax_node()),
            )]),
        ))
    }

    fn handle_component_functions(&mut self, db: &dyn SyntaxGroup, func: ast::FunctionWithBody) {
        let declaration = func.declaration(db);

        let mut func_declaration = RewriteNode::from_ast(&declaration);
        func_declaration
            .modify_child(db, ast::FunctionDeclaration::INDEX_SIGNATURE)
            .modify_child(db, ast::FunctionSignature::INDEX_PARAMETERS)
            .modify(db)
            .children
            .splice(0..1, vec![RewriteNode::Text("entity_id: felt".to_string())]);

        self.rewrite_nodes.push(RewriteNode::interpolate_patched(
            "
                            #[view]
                            $func_decl$ {
                                let self = state::read(entity_id);
                                $body$
                            }
                            ",
            HashMap::from([
                ("func_decl".to_string(), func_declaration),
                (
                    "body".to_string(),
                    RewriteNode::Trimmed(func.body(db).statements(db).as_syntax_node()),
                ),
            ]),
        ))
    }
}

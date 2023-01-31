use std::collections::HashMap;
use std::sync::Arc;
use std::vec;

use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, GeneratedFileAuxData, MacroPlugin, PluginDiagnostic,
    PluginGeneratedFile, PluginResult,
};
use cairo_lang_diagnostics::DiagnosticEntry;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::patcher::{ModifiedNode, PatchBuilder, Patches, RewriteNode};
use cairo_lang_semantic::plugin::{
    AsDynGeneratedFileAuxData, AsDynMacroPlugin, DiagnosticMapper, DynDiagnosticMapper,
    PluginMappedDiagnostic, SemanticPlugin,
};
use cairo_lang_semantic::SemanticDiagnostic;
use cairo_lang_starknet::contract::starknet_keccak;
use cairo_lang_syntax::node::ast::{MaybeImplBody, MaybeModuleBody};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use indoc::formatdoc;
use itertools::Itertools;

const COMPONENT_ATTR: &str = "component";

/// The diagnostics remapper of the plugin.
#[derive(Debug, PartialEq, Eq)]
pub struct DiagnosticRemapper {
    patches: Patches,
}
impl GeneratedFileAuxData for DiagnosticRemapper {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn eq(&self, other: &dyn GeneratedFileAuxData) -> bool {
        if let Some(other) = other.as_any().downcast_ref::<Self>() {
            self == other
        } else {
            false
        }
    }
}
impl AsDynGeneratedFileAuxData for DiagnosticRemapper {
    fn as_dyn_macro_token(&self) -> &(dyn GeneratedFileAuxData + 'static) {
        self
    }
}
impl DiagnosticMapper for DiagnosticRemapper {
    fn map_diag(
        &self,
        db: &(dyn SemanticGroup + 'static),
        diag: &dyn std::any::Any,
    ) -> Option<PluginMappedDiagnostic> {
        let Some(diag) = diag.downcast_ref::<SemanticDiagnostic>() else {return None;};
        let span = self
            .patches
            .translate(db.upcast(), diag.stable_location.diagnostic_location(db.upcast()).span)?;
        Some(PluginMappedDiagnostic { span, message: diag.format(db) })
    }
}

#[cfg(test)]
#[path = "plugin_test.rs"]
mod test;

#[derive(Debug)]
pub struct DojoPlugin {}

impl MacroPlugin for DojoPlugin {
    fn generate_code(&self, db: &dyn SyntaxGroup, item_ast: ast::Item) -> PluginResult {
        match item_ast {
            ast::Item::Module(module_ast) => handle_mod(db, module_ast),
            ast::Item::FreeFunction(function_ast) => handle_function(db, function_ast),
            // Remove other items.
            _ => PluginResult { remove_original_item: true, ..PluginResult::default() },
        }
    }
}

impl AsDynMacroPlugin for DojoPlugin {
    fn as_dyn_macro_plugin<'a>(self: Arc<Self>) -> Arc<dyn MacroPlugin + 'a>
    where
        Self: 'a,
    {
        self
    }
}
impl SemanticPlugin for DojoPlugin {}

fn handle_mod(db: &dyn SyntaxGroup, module_ast: ast::ItemModule) -> PluginResult {
    if !module_ast.has_attr(db, COMPONENT_ATTR) {
        // TODO: diagnostic
        return PluginResult::default();
    }

    let name = module_ast.name(db).text(db);

    let body = match module_ast.body(db) {
        MaybeModuleBody::Some(body) => body,
        MaybeModuleBody::None(empty_body) => {
            return PluginResult {
                code: None,
                diagnostics: vec![PluginDiagnostic {
                    message: "Modules without body are not supported.".to_string(),
                    stable_ptr: empty_body.stable_ptr().untyped(),
                }],
                remove_original_item: false,
            };
        }
    };
    let mut diagnostics = vec![];
    let mut rewrite_nodes: Vec<RewriteNode> = vec![];

    for item in body.items(db).elements(db) {
        match &item {
            ast::Item::Struct(item_struct) => {
                // TODO: verify only one struct per module.
                let rewrite_node = handle_component(db, item_struct.clone());
                rewrite_nodes.push(RewriteNode::Copied(item_struct.as_syntax_node()));
                rewrite_nodes.push(rewrite_node);
            }
            ast::Item::FreeFunction(item_function) => {
                let rewrite_node = handle_component_functions(db, item_function.clone());
                rewrite_nodes.push(rewrite_node);
            }
            _ => (),
        }
    }

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
            RewriteNode::Modified(ModifiedNode { children: rewrite_nodes }),
        )]),
    ));

    PluginResult {
        code: Some(PluginGeneratedFile {
            name: "contract".into(),
            content: builder.code,
            aux_data: DynGeneratedFileAuxData::new(DynDiagnosticMapper::new(DiagnosticRemapper {
                patches: builder.patches,
            })),
        }),
        diagnostics,
        remove_original_item: true,
    }
}

fn handle_function(db: &dyn SyntaxGroup, function_ast: ast::FunctionWithBody) -> PluginResult {
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
        _ => return PluginResult::default(),
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

    let mut functions = vec![];
    functions.push(RewriteNode::interpolate_patched(
        &formatdoc!(
            "
            struct Storage {{
                world_address: felt,
            }}

            #[external]
            fn initialize(world_addr: felt, component_ids: Array::<felt>) {{
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

    let diagnostics = vec![];
    let mut builder = PatchBuilder::new(db);
    builder.add_modified(RewriteNode::interpolate_patched(
        &formatdoc!(
            "
            #[contract]
            mod {system_name} {{
                $body$
            }}
            "
        ),
        HashMap::from([(
            "body".to_string(),
            RewriteNode::Modified(ModifiedNode { children: functions }),
        )]),
    ));
    PluginResult {
        code: Some(PluginGeneratedFile {
            name: system_name.into(),
            content: builder.code,
            aux_data: DynGeneratedFileAuxData::new(DynDiagnosticMapper::new(DiagnosticRemapper {
                patches: builder.patches,
            })),
        }),
        diagnostics,
        remove_original_item: true,
    }
}

fn handle_component(db: &dyn SyntaxGroup, struct_ast: ast::ItemStruct) -> RewriteNode {
    RewriteNode::interpolate_patched(
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
    )
}

fn handle_component_functions(db: &dyn SyntaxGroup, func: ast::FunctionWithBody) -> RewriteNode {
    let declaration = func.declaration(db);

    let mut func_declaration = RewriteNode::from_ast(&declaration);
    func_declaration
        .modify_child(db, ast::FunctionDeclaration::INDEX_SIGNATURE)
        .modify_child(db, ast::FunctionSignature::INDEX_PARAMETERS)
        .modify(db)
        .children
        .splice(0..1, vec![RewriteNode::Text("entity_id: felt".to_string())]);

    RewriteNode::interpolate_patched(
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
    )
}

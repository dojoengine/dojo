use std::collections::HashMap;
use std::sync::Arc;
use std::vec;

use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, GeneratedFileAuxData, MacroPlugin, PluginGeneratedFile, PluginResult,
};
use cairo_lang_diagnostics::DiagnosticEntry;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::patcher::{ModifiedNode, PatchBuilder, Patches, RewriteNode};
use cairo_lang_semantic::plugin::{
    AsDynGeneratedFileAuxData, AsDynMacroPlugin, DiagnosticMapper, DynDiagnosticMapper,
    PluginMappedDiagnostic, SemanticPlugin,
};
use cairo_lang_semantic::SemanticDiagnostic;
use cairo_lang_sierra_generator::pre_sierra::Function;
use cairo_lang_syntax::node::ast::{OptionWrappedGenericParamListEmpty, WrappedGenericParamList};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use indoc::formatdoc;

const COMPONENT_TRAIT: &str = "Component";
const SYSTEM_TRAIT: &str = "System";

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
            ast::Item::Struct(struct_ast) => handle_struct(db, struct_ast),
            ast::Item::Impl(impl_ast) => handle_impl(db, impl_ast),
            ast::Item::FreeFunction(function_ast) => handle_function(db, function_ast),
            // ast::Item::Trait(trait_ast) => handle_trait(db, trait_ast),
            // Nothing to do for other items.
            _ => PluginResult::default(),
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

/// If the trait is annotated with SYSTEM_TRAIT, generate the relevant dispatcher logic.
fn handle_function(db: &dyn SyntaxGroup, function_ast: ast::ItemFreeFunction) -> PluginResult {
    let attrs = function_ast.attributes(db).elements(db);

    for attr in attrs {
        if attr.attr(db).text(db) == "derive" {
            if let ast::OptionAttributeArgs::AttributeArgs(args) = attr.args(db) {
                for arg in args.arg_list(db).elements(db) {
                    if let ast::Expr::Path(expr) = arg {
                        if let [ast::PathSegment::Simple(segment)] = &expr.elements(db)[..] {
                            if segment.ident(db).text(db).as_str() != SYSTEM_TRAIT {
                                return PluginResult::default();
                            }

                            return handle_system(db, function_ast);
                        }
                    }
                }
            }
        }
    }

    PluginResult::default()
}

/// If the trait is annotated with COMPONENT_TRAIT, generate the relevant dispatcher logic.
fn handle_struct(db: &dyn SyntaxGroup, struct_ast: ast::ItemStruct) -> PluginResult {
    let attrs = struct_ast.attributes(db).elements(db);

    for attr in attrs {
        if attr.attr(db).text(db) == "derive" {
            if let ast::OptionAttributeArgs::AttributeArgs(args) = attr.args(db) {
                for arg in args.arg_list(db).elements(db) {
                    if let ast::Expr::Path(expr) = arg {
                        if let [ast::PathSegment::Simple(segment)] = &expr.elements(db)[..] {
                            if segment.ident(db).text(db).as_str() != COMPONENT_TRAIT {
                                return PluginResult::default();
                            }

                            return handle_component(db, struct_ast);
                        }
                    }
                }
            }
        }
    }

    PluginResult::default()
}

fn handle_system(db: &dyn SyntaxGroup, function_ast: ast::ItemFreeFunction) -> PluginResult {
    let system_name = format!("{}System", function_ast.declaration(db).name(db).text(db));
    let signature = function_ast.declaration(db).signature(db);
    let parameters = signature.parameters(db).elements(db);

    let query_param = parameters
        .iter()
        .find(|attr| match attr.name(db) {
            ast::ParamName::Underscore(_) => false,
            ast::ParamName::Name(name) => name.text(db).as_str() == "query",
        })
        .unwrap();

    let generic_type;
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

            generic_type = generic.generic_args(db).elements(db)[0].clone();
        }
        _ => return PluginResult::default(),
    }

    // let generic_type;
    // match query_param.type_clause(db).ty(db) {
    //     ast::Expr::Path(path) => {
    //         let generic = path.elements(db).iter().find_map(|segment| {
    //             match segment {
    //                 ast::PathSegment::WithGenericArgs(segment) => {
    //                     if segment.ident(db).text(db).as_str() == "Query" {
    //                         Some(segment.generic_args(db))
    //                     } else {
    //                         None
    //                     }
    //                 },
    //                 _ => None,
    //             }
    //         }).unwrap();

    //         generic_type = generic.generic_args(db).elements(db)[0];

    //     },
    //     _ => return PluginResult::default(),
    // }

    // let generic;
    // match function_ast.declaration(db).generic_params(db) {
    //     ast::OptionWrappedGenericParamList::Empty(_) => {
    //         println!("No generic params");
    //         return PluginResult::default();
    //     },
    //     ast::OptionWrappedGenericParamList::WrappedGenericParamList(params) => {
    //         let generics = params.generic_params(db).elements(db);
    //         generic = generics[0].clone();
    //     },
    // }

    let generic_name = generic_type.as_syntax_node().get_text(db);

    let mut functions = vec![];
    functions.push(RewriteNode::interpolate_patched(
        format!(
            "struct Storage {{
        world_address: felt,
    }}

    #[external]
    fn initialize(world_addr: felt) {{
        let res = world_address::read();
        match res {{
            Option::Some(_) => {{
                let mut err_data = array_new::<felt>();
                array_append::<felt>(err_data, '{system_name}: Already initialized.');
                panic(err_data);
            }},
            Option::None(_) => {{
                world_address::write(world_addr);
            }},
        }}
    }}
    
    impl {generic_name}Query of Query::<$generic_type$> {{
        fn iter() -> Map::<felt, $generic_type$> {{
            // TODO: World contract dispatcher and get component and entities.
        }}
    }}

    #[external]
    fn execute(query: {generic_name}Query) {{
        let world = world_address::read();
        match world {{
            Option::None(_) => {{
                let mut err_data = array_new::<felt>();
                array_append::<felt>(err_data, '{system_name}: Not initialized.');
                panic(err_data);
            }},
        }}
        
        $body$
    }}"
        )
        .as_str(),
        HashMap::from([
            (
                "type_name".to_string(),
                RewriteNode::Trimmed(function_ast.declaration(db).name(db).as_syntax_node()),
            ),
            ("generic_type".to_string(), RewriteNode::Trimmed(generic_type.as_syntax_node())),
            ("body".to_string(), RewriteNode::Trimmed(function_ast.body(db).statements(db).as_syntax_node())),
            ("query_param".to_string(), RewriteNode::Trimmed(query_param.as_syntax_node())),
            // ("parameters".to_string(), RewriteNode::Trimmed(function_ast.declaration(db).signature(db).parameters(db).as_syntax_node())),
        ]),
    ));

    let diagnostics = vec![];
    let mut builder = PatchBuilder::new(db);
    builder.add_modified(RewriteNode::interpolate_patched(
        &formatdoc!(
            "#[contract]
                 mod {system_name} {{
                     $body$
                 }}",
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
        remove_original_item: false,
    }
}

fn handle_component(db: &dyn SyntaxGroup, struct_ast: ast::ItemStruct) -> PluginResult {
    let mut functions = vec![];
    functions.push(RewriteNode::interpolate_patched(
        format!(
            "struct Storage {{
        world_address: felt,
        $storage_var_name$s: Map::<felt, $type_name$>,
     }}

     // Initialize $type_name$Component.
     #[external]
     fn initialize(world_addr: felt) {{
         let res = world_address::read();
         match res {{
             Option::Some(_) => {{
                 let mut err_data = array_new::<felt>();
                 array_append::<felt>(err_data, '$type_name$Component: Already initialized.');
                 panic(err_data);
             }},
             Option::None(_) => {{
                world_address::write(world_addr);
             }},
         }}
     }}

     // Set the $storage_var_name$ of an entity.
     #[external]
     fn set(entity_id: felt, value: $type_name$) {{
         let res = $storage_var_name$s::read();
         $storage_var_name$s::write(entity_id, value);
     }}

     // Get the $storage_var_name$ of an entity.
     #[view]
     fn get(entity_id: felt) -> $type_name$ {{
         return $storage_var_name$s::read(entity_id);
     }}"
        )
        .as_str(),
        HashMap::from([
            ("type_name".to_string(), RewriteNode::Trimmed(struct_ast.name(db).as_syntax_node())),
            (
                "storage_var_name".to_string(),
                RewriteNode::Text(struct_ast.name(db).text(db).to_lowercase()),
            ),
        ]),
    ));

    let diagnostics = vec![];
    let mut builder = PatchBuilder::new(db);
    let component_name = format!("{}Component", struct_ast.name(db).text(db));
    builder.add_modified(RewriteNode::interpolate_patched(
        &formatdoc!(
            "#[contract]
             mod {component_name} {{
                 $body$
             }}",
        ),
        HashMap::from([(
            "body".to_string(),
            RewriteNode::Modified(ModifiedNode { children: functions }),
        )]),
    ));
    PluginResult {
        code: Some(PluginGeneratedFile {
            name: component_name.into(),
            content: builder.code,
            aux_data: DynGeneratedFileAuxData::new(DynDiagnosticMapper::new(DiagnosticRemapper {
                patches: builder.patches,
            })),
        }),
        diagnostics,
        remove_original_item: false,
    }
}

fn handle_impl(_db: &dyn SyntaxGroup, _impl_ast: ast::ItemImpl) -> PluginResult {
    PluginResult { remove_original_item: true, ..PluginResult::default() }
}

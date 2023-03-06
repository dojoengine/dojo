use std::collections::HashMap;

use cairo_lang_defs::ids::{ModuleItemId, SubmoduleId};
use cairo_lang_defs::plugin::{
    DynGeneratedFileAuxData, PluginDiagnostic, PluginGeneratedFile, PluginResult,
};
use cairo_lang_filesystem::ids::CrateId;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::patcher::{PatchBuilder, RewriteNode};
use cairo_lang_semantic::plugin::DynPluginAuxData;
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::{ast, Terminal, TypedSyntaxNode};
use cairo_lang_utils::try_extract_matches;
use dojo_project::WorldConfig;
use indoc::formatdoc;
use smol_str::SmolStr;

use crate::plugin::DojoAuxData;
use crate::query::Query;

#[cfg(test)]
#[path = "system_test.rs"]
mod test;

/// Represents a declaration of a system.
pub struct SystemDeclaration {
    /// The id of the module that defines the system.
    pub submodule_id: SubmoduleId,
    pub name: SmolStr,
}

pub struct System {
    pub name: SmolStr,
    pub world_config: WorldConfig,
    pub rewrite_nodes: Vec<RewriteNode>,
    pub diagnostics: Vec<PluginDiagnostic>,
}

impl System {
    pub fn from_module_body(
        db: &dyn SyntaxGroup,
        world_config: WorldConfig,
        name: SmolStr,
        body: ast::ModuleBody,
    ) -> Self {
        let diagnostics = vec![];
        let rewrite_nodes: Vec<RewriteNode> = vec![];
        let mut system = System { name, world_config, rewrite_nodes, diagnostics };

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
                    use dojo::world;
                    $body$
                }}
                ",
            ),
            HashMap::from([("body".to_string(), RewriteNode::new_modified(self.rewrite_nodes))]),
        ));

        PluginResult {
            code: Some(PluginGeneratedFile {
                name: name.clone(),
                content: builder.code,
                aux_data: DynGeneratedFileAuxData::new(DynPluginAuxData::new(DojoAuxData {
                    patches: builder.patches,
                    components: vec![],
                    systems: vec![name],
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
                let query = Query::from_expr(db, self.world_config, type_ast.clone());
                preprocess_rewrite_nodes.extend(query.rewrite_nodes);
            }
        }

        self.rewrite_nodes.push(RewriteNode::interpolate_patched(
            &formatdoc!(
                "
                struct Storage {{
                }}
    
                #[external]
                fn execute() {{
                    let world_address = starknet::contract_address_const::<$world_address$>();
                    $preprocessing$
                    $body$
                }}
                "
            ),
            HashMap::from([
                (
                    "body".to_string(),
                    RewriteNode::new_trimmed(function_ast.body(db).statements(db).as_syntax_node()),
                ),
                ("preprocessing".to_string(), RewriteNode::new_modified(preprocess_rewrite_nodes)),
                (
                    "world_address".to_string(),
                    RewriteNode::Text(format!(
                        "{:#x}",
                        self.world_config.address.unwrap_or_default()
                    )),
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

    if ty == "Query" { Some(SystemArgType::Query) } else { None }
}

/// Finds the inline modules annotated as systems in the given crate_ids and
/// returns the corresponding SystemDeclarations.
pub fn find_systems(db: &dyn SemanticGroup, crate_ids: &[CrateId]) -> Vec<SystemDeclaration> {
    let mut systems = vec![];
    for crate_id in crate_ids {
        let modules = db.crate_modules(*crate_id);
        for module_id in modules.iter() {
            let generated_file_infos =
                db.module_generated_file_infos(*module_id).unwrap_or_default();

            for generated_file_info in generated_file_infos.iter().skip(1) {
                let Some(generated_file_info) = generated_file_info else { continue; };
                let Some(mapper) = generated_file_info.aux_data.0.as_any(
                ).downcast_ref::<DynPluginAuxData>() else { continue; };
                let Some(aux_data) = mapper.0.as_any(
                ).downcast_ref::<DojoAuxData>() else { continue; };

                for name in &aux_data.systems {
                    if let Ok(Some(ModuleItemId::Submodule(submodule_id))) =
                        db.module_item_by_name(*module_id, name.clone())
                    {
                        systems.push(SystemDeclaration { name: name.clone(), submodule_id });
                    } else {
                        panic!("System `{name}` was not found.");
                    }
                }
            }
        }
    }
    systems
}

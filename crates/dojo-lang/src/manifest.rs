use std::collections::HashMap;

use anyhow::Context;
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_debug::debug::DebugWithDb; // use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_defs::db::DefsGroup;
use cairo_lang_defs::ids::{FunctionWithBodyId, ModuleFileId, ModuleId, ModuleItemId};
use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_filesystem::ids::CrateId;
use cairo_lang_semantic as semantic;
use cairo_lang_starknet::abi;
use cairo_lang_starknet::plugin::aux_data::StarkNetContractAuxData;
use cairo_lang_syntax::node::ast::{Expr, ExprPath, ExprStructCtorCall, StatementExpr};
use cairo_lang_syntax::node::kind::SyntaxKind;
use cairo_lang_syntax::node::{SyntaxNode, TypedSyntaxNode};
use convert_case::{Case, Casing};
use dojo_world::manifest::{
    Class, Contract, BASE_CONTRACT_NAME, EXECUTOR_CONTRACT_NAME, WORLD_CONTRACT_NAME,
};
use semantic::db::SemanticGroup;
use semantic::expr::compute::{maybe_compute_expr_semantic, ComputationContext};
use semantic::expr::fmt::ExprFormatter;
use semantic::items::function_with_body::SemanticExprLookup;
use semantic::resolve::Resolver;
use semantic::types::resolve_type;
use serde::Serialize;
use smol_str::SmolStr;
use starknet::core::types::FieldElement;

use crate::inline_macros::set::WRITERS;
use crate::plugin::DojoAuxData;

#[derive(Default, Debug, Serialize)]
pub(crate) struct Manifest(dojo_world::manifest::Manifest);

impl Manifest {
    pub fn new(
        db: &RootDatabase,
        crate_ids: &[CrateId],
        compiled_classes: HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
    ) -> Self {
        let mut manifest = Manifest(dojo_world::manifest::Manifest::default());
        let (world, world_abi) = compiled_classes.get(WORLD_CONTRACT_NAME).unwrap_or_else(|| {
            panic!(
                "{}",
                format!(
                    "Contract `{}` not found. Did you include `dojo` as a dependency?",
                    WORLD_CONTRACT_NAME
                )
            );
        });

        // let green_id = GreenId::new();
        let (executor, executor_abi) =
            compiled_classes.get(EXECUTOR_CONTRACT_NAME).unwrap_or_else(|| {
                panic!(
                    "{}",
                    format!(
                        "Contract `{}` not found. Did you include `dojo` as a dependency?",
                        EXECUTOR_CONTRACT_NAME
                    )
                );
            });
        let (base, base_abi) = compiled_classes.get(BASE_CONTRACT_NAME).unwrap_or_else(|| {
            panic!(
                "{}",
                format!(
                    "Contract `{}` not found. Did you include `dojo` as a dependency?",
                    BASE_CONTRACT_NAME
                )
            );
        });

        manifest.0.world = Contract {
            name: WORLD_CONTRACT_NAME.into(),
            address: None,
            class_hash: *world,
            abi: world_abi.clone(),
            ..Contract::default()
        };
        manifest.0.base =
            Class { name: BASE_CONTRACT_NAME.into(), class_hash: *base, abi: base_abi.clone() };
        manifest.0.executor = Contract {
            name: EXECUTOR_CONTRACT_NAME.into(),
            address: None,
            class_hash: *executor,
            abi: executor_abi.clone(),
            ..Contract::default()
        };

        // let ctx =
        //     ComputationContext::new(db, diagnostics, function, resolver, signature, environment);
        // println!("{writers:#?}");
        // WRITERS.iter().for_each(|(module, node)| match node.kind(db) {
        //     SyntaxKind::ExprPath => {
        //         let expr = Expr::Path(ExprPath::from_syntax_node(db, node.clone()));
        //         let ptr = expr.stable_ptr();
        //         // db.lookup_intern_free_function();
        //         // db.lookup_expr_by_ptr(function_id, ptr.into());

        //         // let com = maybe_compute_expr_semantic(ctx, expr);

        //         let component = expr.as_syntax_node().get_text_without_trivia(db);
        //         println!("{} {}", module, component);
        //     }
        //     SyntaxKind::ExprStructCtorCall => {
        //         let expr = ExprStructCtorCall::from_syntax_node(db, node.clone());
        //         let component = expr.path(db).as_syntax_node().get_text_without_trivia(db);
        //         println!("{} {}", module, component);
        //     }
        //     _ => eprintln!("Unsupport type {}", node.kind(db)),
        // });

        for crate_id in crate_ids {
            let modules = db.crate_modules(*crate_id);

            for module_id in modules.iter() {
                let generated_file_infos =
                    db.module_generated_file_infos(*module_id).unwrap_or_default();
                for generated_file_info in generated_file_infos.iter().skip(1) {
                    let Some(generated_file_info) = generated_file_info else {
                        continue;
                    };
                    let Some(aux_data) = &generated_file_info.aux_data else {
                        continue;
                    };
                    let aux_data = aux_data.0.as_any();

                    if let Some(dojo_aux_data) = aux_data.downcast_ref() {
                        manifest.find_models(db, dojo_aux_data, *module_id, &compiled_classes);
                    }
                    if let Some(contracts) = aux_data.downcast_ref::<StarkNetContractAuxData>() {
                        manifest.find_contracts(db, module_id, contracts, &compiled_classes);
                    }
                }
            }
            manifest.filter_contracts();
        }

        manifest
    }

    /// Finds the inline modules annotated as models in the given crate_ids and
    /// returns the corresponding Models.
    fn find_models(
        &mut self,
        db: &dyn SemanticGroup,
        aux_data: &DojoAuxData,
        module_id: ModuleId,
        compiled_classes: &HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
    ) {
        for model in &aux_data.models {
            let model = model.clone();
            let name: SmolStr = model.name.clone().into();
            if let Ok(Some(ModuleItemId::Struct(_))) =
                db.module_item_by_name(module_id, name.clone())
            {
                // It needs the `Model` suffix because we are
                // searching from the compiled contracts.
                let (class_hash, class_abi) = compiled_classes
                    .get(name.to_case(Case::Snake).as_str())
                    .with_context(|| format!("Model {name} not found in target."))
                    .unwrap();

                self.0.models.push(dojo_world::manifest::Model {
                    name: model.name,
                    members: model.members,
                    class_hash: *class_hash,
                    abi: class_abi.clone(),
                });
            }
        }
    }

    // removes contracts with DojoAuxType
    fn filter_contracts(&mut self) {
        let mut models = HashMap::new();

        for model in &self.0.models {
            models.insert(model.class_hash, true);
        }

        for i in (0..self.0.contracts.len()).rev() {
            if models.get(&self.0.contracts[i].class_hash).is_some() {
                self.0.contracts.remove(i);
            }
        }
    }

    fn find_contracts(
        &mut self,
        db: &RootDatabase,
        module_id: &ModuleId,
        aux_data: &StarkNetContractAuxData,
        compiled_classes: &HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
    ) {
        for name in &aux_data.contracts {
            if "world" == name.as_str() || "executor" == name.as_str() || "base" == name.as_str() {
                return;
            }

            let (class_hash, abi) = compiled_classes.get(name).unwrap().clone();

            let module_name = module_id.full_path(db);
            let module_last_name = module_name.split("::").last().unwrap();
            println!("--------------------\n{module_name}");

            let writers = WRITERS.lock().unwrap();
            let deps = self.find_module_writes(db, module_id, writers.get(module_last_name));

            println!("Components: {:?}", deps);

            self.0.contracts.push(Contract {
                name: name.clone(),
                address: None,
                class_hash,
                abi,
                deps,
            });
        }
    }

    fn find_function_writes(
        &self,
        db: &RootDatabase,
        module_id: &ModuleId,
        module_writers: &HashMap<String, Vec<SyntaxNode>>,
        fn_id: FunctionWithBodyId,
        components: &mut HashMap<String, bool>,
    ) {
        let fn_body = db.function_body(fn_id).unwrap();
        let fn_name: String = fn_id.name(db).into();
        let fn_expr = db.expr_semantic(fn_id, fn_body.body_expr);
        // println!("Parsing ..::{}()", fn_name);
        // module_fn_map.insert(fn_name, *fn_id);
        if let Some(module_fn_writers) = module_writers.get(&fn_name) {
            // This functions has writers
            // Do stuff with the writers
            println!("Writes found: {}", module_fn_writers.len());
            println!("\nFunction {fn_name}: \n{:#?}\n\n", fn_expr);
            let expr_formatter = ExprFormatter { db, function_id: fn_id };
            println!("{:#?}", fn_expr.debug(&expr_formatter));

            for node in module_fn_writers.iter() {
                match node.kind(db) {
                    SyntaxKind::ExprPath => {
                        let expr = Expr::Path(ExprPath::from_syntax_node(db, node.clone()));
                        // let ptr = expr.stable_ptr();
                        // db.lookup_intern_free_function();
                        // db.lookup_expr_by_ptr(function_id, ptr.into());
                        // let fn_expr = db.function_body_expr(fn_id).unwrap();
                        // let diagnostics = db.function_body_diagnostics(fn_id);
                        let expr_semantic = db.lookup_expr_by_ptr(fn_id, expr.stable_ptr());
                        // resolve_type(db, diagnostics, resolver, ty_syntax);
                        let crate_id = module_id.owning_crate(db);
                        // let module_file = ModuleFileId

                        // let mut diagnostics = SemanticDiagnostics::new(module_id);

                        // let ctx = ComputationContext::new(
                        //     db,
                        //     diagnostics,
                        //     Some(fn_body),
                        //     Resolver::new(db, module_file_id, inference_id),
                        //     signature,
                        //     environment,
                        // );
                        // maybe_compute_expr_semantic(ctx, syntax);

                        // println!("{:?} {:?}", expr_semantic, diagnostic);

                        let component = expr.as_syntax_node().get_text_without_trivia(db);
                        components.insert(component, true);
                    }
                    SyntaxKind::StatementExpr => {
                        let expr = StatementExpr::from_syntax_node(db, node.clone());
                        // let ptr = expr.stable_ptr();
                        // db.lookup_intern_free_function();
                        // db.lookup_expr_by_ptr(function_id, ptr.into());
                        // let fn_expr = db.function_body_expr(fn_id).unwrap();
                        let diagnostic = db.function_body_diagnostics(fn_id);
                        // let expr_semantic = db.lookup_expr_by_ptr(fn_id, expr.stable_ptr());

                        println!("StatementExpr{{:?}}:\n{}", node.get_text(db));

                        let component = expr.as_syntax_node().get_text_without_trivia(db);
                        components.insert(component, true);
                    }
                    SyntaxKind::ExprStructCtorCall => {
                        let expr = ExprStructCtorCall::from_syntax_node(db, node.clone());
                        let component = expr.path(db).as_syntax_node().get_text_without_trivia(db);
                        components.insert(component, true);
                    }
                    _ => eprintln!(
                        "Unsupport component value type {} for semantic writer analysis",
                        node.kind(db)
                    ),
                }
            }
            println!("");
        }
    }

    fn find_module_writes(
        &self,
        db: &RootDatabase,
        module_id: &ModuleId,
        module_writers: Option<&HashMap<String, Vec<SyntaxNode>>>,
    ) -> Vec<String> {
        let mut components: HashMap<String, bool> = HashMap::new();
        // Does the module have writers?
        if let Some(module_writers) = module_writers {
            // Get module fn ids. And generate lookup hashmap
            if let Ok(module_fns) = db.module_free_functions_ids(*module_id) {
                for fn_id in module_fns.iter() {
                    self.find_function_writes(
                        db,
                        module_id,
                        module_writers,
                        FunctionWithBodyId::Free(*fn_id),
                        &mut components,
                    );
                }
            }
            if let Ok(module_impls) = db.module_impls_ids(*module_id) {
                for module_impl_id in module_impls.iter() {
                    if let Ok(module_fns) = db.impl_functions(*module_impl_id) {
                        for (_, fn_id) in module_fns.iter() {
                            self.find_function_writes(
                                db,
                                module_id,
                                module_writers,
                                FunctionWithBodyId::Impl(*fn_id),
                                &mut components,
                            );
                        }
                    }
                }
            }
        }

        components.into_iter().map(|(component, _)| component).collect()
    }
}

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

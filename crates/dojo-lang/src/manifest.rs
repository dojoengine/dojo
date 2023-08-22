use std::collections::HashMap;

use anyhow::{Context, Result};
use cairo_lang_defs::ids::{ModuleId, ModuleItemId};
use cairo_lang_filesystem::ids::CrateId;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::plugin::DynPluginAuxData;
use cairo_lang_starknet::abi;
use convert_case::{Case, Casing};
use dojo_world::manifest::{
    Contract, Input, Output, System, EXECUTOR_CONTRACT_NAME, WORLD_CONTRACT_NAME,
};
use itertools::Itertools;
use serde::Serialize;
use smol_str::SmolStr;
use starknet::core::types::FieldElement;

use crate::plugin::{DojoAuxData, SystemAuxData};

#[derive(Default, Debug, Serialize)]
pub(crate) struct Manifest(dojo_world::manifest::Manifest);

impl Manifest {
    pub fn new(
        db: &dyn SemanticGroup,
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

        manifest.0.world = Contract {
            name: WORLD_CONTRACT_NAME.into(),
            address: None,
            class_hash: *world,
            abi: world_abi.clone(),
        };
        manifest.0.executor = Contract {
            name: EXECUTOR_CONTRACT_NAME.into(),
            address: None,
            class_hash: *executor,
            abi: executor_abi.clone(),
        };

        for crate_id in crate_ids {
            let modules = db.crate_modules(*crate_id);
            for module_id in modules.iter() {
                let generated_file_infos =
                    db.module_generated_file_infos(*module_id).unwrap_or_default();

                for generated_file_info in generated_file_infos.iter().skip(1) {
                    let Some(generated_file_info) = generated_file_info else {
                        continue;
                    };
                    let Some(mapper) =
                        generated_file_info.aux_data.0.as_any().downcast_ref::<DynPluginAuxData>()
                    else {
                        continue;
                    };
                    let Some(aux_data) = mapper.0.as_any().downcast_ref::<DojoAuxData>() else {
                        continue;
                    };

                    manifest.find_components(db, aux_data, *module_id, &compiled_classes);
                    manifest.find_systems(db, aux_data, *module_id, &compiled_classes).unwrap();
                }
            }
        }

        manifest
    }

    /// Finds the inline modules annotated as components in the given crate_ids and
    /// returns the corresponding Components.
    fn find_components(
        &mut self,
        db: &dyn SemanticGroup,
        aux_data: &DojoAuxData,
        module_id: ModuleId,
        compiled_classes: &HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
    ) {
        for component in &aux_data.components {
            let component = component.clone();
            let name: SmolStr = component.name.clone().into();
            if let Ok(Some(ModuleItemId::Struct(_))) =
                db.module_item_by_name(module_id, name.clone())
            {
                // It needs the `Component` suffix because we are
                // searching from the compiled contracts.
                let (class_hash, class_abi) = compiled_classes
                    .get(name.to_case(Case::Snake).as_str())
                    .with_context(|| format!("Component {name} not found in target."))
                    .unwrap();

                self.0.components.push(dojo_world::manifest::Component {
                    name: component.name,
                    members: component.members,
                    class_hash: *class_hash,
                    abi: class_abi.clone(),
                });
            }
        }
    }

    fn find_systems(
        &mut self,
        db: &dyn SemanticGroup,
        aux_data: &DojoAuxData,
        module_id: ModuleId,
        compiled_classes: &HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
    ) -> Result<()> {
        for SystemAuxData { name, dependencies } in &aux_data.systems {
            if let Ok(Some(ModuleItemId::Submodule(submodule_id))) =
                db.module_item_by_name(module_id, name.clone())
            {
                let defs_db = db.upcast();
                let fns = db.module_free_functions_ids(ModuleId::Submodule(submodule_id)).unwrap();
                for fn_id in fns {
                    if fn_id.name(defs_db) != "execute" {
                        continue;
                    }
                    let signature = db.free_function_signature(fn_id).unwrap();

                    let mut inputs = vec![];
                    let mut params = signature.params;

                    // Last arg is always the `world_address` which is provided by the executor.
                    params.pop();
                    for param in params.into_iter() {
                        let ty = param.ty.format(db);
                        // Context is injected by the executor contract.
                        if ty == "dojo::world::Context" {
                            continue;
                        }

                        inputs.push(Input { name: param.id.name(db.upcast()).into(), ty });
                    }

                    let outputs = if signature.return_type.is_unit(db) {
                        vec![]
                    } else {
                        vec![Output { ty: signature.return_type.format(db) }]
                    };

                    let (class_hash, class_abi) = compiled_classes
                        .get(name.as_str())
                        .with_context(|| format!("System {name} not found in target."))
                        .unwrap();

                    self.0.systems.push(System {
                        name: name.clone(),
                        inputs,
                        outputs,
                        class_hash: *class_hash,
                        dependencies: dependencies
                            .iter()
                            .sorted_by(|a, b| a.name.cmp(&b.name))
                            .cloned()
                            .collect::<Vec<_>>(),
                        abi: class_abi.clone(),
                    });
                }
            } else {
                panic!("System `{name}` was not found.");
            }
        }

        Ok(())
    }
}

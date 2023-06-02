use std::collections::HashMap;

use anyhow::{Context, Result};
use cairo_lang_defs::ids::{ModuleId, ModuleItemId};
use cairo_lang_filesystem::ids::CrateId;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::plugin::DynPluginAuxData;
use dojo_world::manifest::{Input, Output, System};
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
        compiled_classes: HashMap<SmolStr, FieldElement>,
    ) -> Self {
        let mut manifest = Manifest(dojo_world::manifest::Manifest::default());

        let world = compiled_classes.get("World").unwrap_or_else(|| {
            panic!("World contract not found. Did you include `dojo_core` as a dependency?");
        });
        let executor = compiled_classes.get("Executor").unwrap_or_else(|| {
            panic!("Executor contract not found. Did you include `dojo_core` as a dependency?");
        });

        manifest.0.world = *world;
        manifest.0.executor = *executor;

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
        compiled_classes: &HashMap<SmolStr, FieldElement>,
    ) {
        for component in &aux_data.components {
            let component = component.clone();
            let name: SmolStr = component.name.clone().into();
            if let Ok(Some(ModuleItemId::Struct(_))) =
                db.module_item_by_name(module_id, name.clone())
            {
                // It needs the `Component` suffix because we are
                // searching from the compiled contracts.
                let class_hash = compiled_classes
                    .get(format!("{name}Component").as_str())
                    .with_context(|| format!("Contract {name} not found in target."))
                    .unwrap();

                self.0.components.push(dojo_world::manifest::Component {
                    name: component.name,
                    members: component.members,
                    class_hash: *class_hash,
                });
            }
        }
    }

    fn find_systems(
        &mut self,
        db: &dyn SemanticGroup,
        aux_data: &DojoAuxData,
        module_id: ModuleId,
        compiled_classes: &HashMap<SmolStr, FieldElement>,
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
                        inputs.push(Input {
                            name: param.id.name(db.upcast()).into(),
                            ty: param.ty.format(db),
                        });
                    }

                    let outputs = if signature.return_type.is_unit(db) {
                        vec![]
                    } else {
                        vec![Output { ty: signature.return_type.format(db) }]
                    };

                    let class_hash = compiled_classes
                        .get(name.as_str())
                        .with_context(|| format!("Contract {name} not found in target."))
                        .unwrap();

                    self.0.systems.push(System {
                        name: name.clone(),
                        inputs,
                        outputs,
                        class_hash: *class_hash,
                        dependencies: dependencies
                            .iter()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>(),
                    });
                }
            } else {
                panic!("System `{name}` was not found.");
            }
        }

        Ok(())
    }
}

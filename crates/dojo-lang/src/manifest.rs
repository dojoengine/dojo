use std::collections::HashMap;

use anyhow::Context;
use cairo_lang_defs::ids::{ModuleId, ModuleItemId};
use cairo_lang_filesystem::ids::CrateId;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_starknet::abi;
use cairo_lang_starknet::plugin::aux_data::StarkNetContractAuxData;
use convert_case::{Case, Casing};
use dojo_world::manifest::{Contract, EXECUTOR_CONTRACT_NAME, WORLD_CONTRACT_NAME};
use serde::Serialize;
use smol_str::SmolStr;
use starknet::core::types::FieldElement;

use crate::plugin::DojoAuxData;

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
                    let Some(aux_data) = &generated_file_info.aux_data else {
                        continue;
                    };
                    let Some(aux_data) = aux_data.0.as_any().downcast_ref() else {
                        if let Some(contracts_data) =
                            aux_data.0.as_any().downcast_ref::<StarkNetContractAuxData>()
                        {
                            manifest.find_contracts(contracts_data, &compiled_classes);
                        } else {
                        }

                        continue;
                    };

                    manifest.find_components(db, aux_data, *module_id, &compiled_classes);
                }
            }
            manifest.filter_contracts();
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

    // removes contracts with DojoAuxType
    fn filter_contracts(&mut self) {
        let mut components = HashMap::new();

        for component in &self.0.components {
            components.insert(component.class_hash, true);
        }

        for i in (0..self.0.contracts.len()).rev() {
            if components.get(&self.0.contracts[i].class_hash).is_some() {
                self.0.contracts.remove(i);
            }
        }
    }

    fn find_contracts(
        &mut self,
        aux_data: &StarkNetContractAuxData,
        compiled_classes: &HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
    ) {
        for name in &aux_data.contracts {
            if "world" == name.as_str() || "executor" == name.as_str() {
                return;
            }

            let (class_hash, abi) = compiled_classes.get(name).unwrap().clone();

            self.0.contracts.push(Contract { name: name.clone(), address: None, class_hash, abi });
        }
    }
}

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

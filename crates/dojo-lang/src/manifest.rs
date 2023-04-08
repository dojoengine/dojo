use std::collections::HashMap;
use std::path::Path;
use std::{fs, iter};

use ::serde::{Deserialize, Serialize};
use anyhow::{anyhow, Context, Result};
use cairo_lang_defs::ids::{ModuleId, ModuleItemId};
use cairo_lang_filesystem::ids::CrateId;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::plugin::DynPluginAuxData;
use serde_with::serde_as;
use smol_str::SmolStr;
use starknet::core::serde::unsigned_field_element::{UfeHex, UfeHexOption};
use starknet::core::types::FieldElement;
use starknet::core::utils::{
    cairo_short_string_to_felt, get_selector_from_name, get_storage_var_address,
};
use starknet::providers::jsonrpc::models::{BlockId, BlockTag, FunctionCall};
use starknet::providers::jsonrpc::{HttpTransport, JsonRpcClient};
use thiserror::Error;
use url::Url;

use crate::plugin::{DojoAuxData, SystemAuxData};

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

#[derive(Error, Debug)]
pub enum ManifestError {
    #[error("Compilation error.")]
    CompilationError,
}

/// Component member.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// Represents a declaration of a component.
#[serde_as]
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    pub members: Vec<Member>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
}

/// System input ABI.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Input {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// System Output ABI.
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Output {
    #[serde(rename = "type")]
    pub ty: String,
}

/// Represents a declaration of a system.
#[serde_as]
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct System {
    pub name: SmolStr,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
    pub dependencies: Vec<String>,
}

#[serde_as]
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Contract {
    pub name: SmolStr,
    #[serde_as(as = "UfeHex")]
    pub class_hash: FieldElement,
}

#[serde_as]
#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Manifest {
    #[serde_as(as = "UfeHexOption")]
    pub world: Option<FieldElement>,
    #[serde_as(as = "UfeHexOption")]
    pub executor: Option<FieldElement>,
    pub components: Vec<Component>,
    pub systems: Vec<System>,
    pub contracts: Vec<Contract>,
}

impl Manifest {
    pub fn new(
        db: &dyn SemanticGroup,
        crate_ids: &[CrateId],
        compiled_classes: HashMap<SmolStr, FieldElement>,
    ) -> Self {
        let mut manifest = Manifest::default();

        let world = compiled_classes.get("World").unwrap_or_else(|| {
            panic!("World contract not found. Did you include `dojo_core` as a dependency?");
        });
        let executor = compiled_classes.get("Executor").unwrap_or_else(|| {
            panic!("Executor contract not found. Did you include `dojo_core` as a dependency?");
        });

        manifest.world = Some(*world);
        manifest.executor = Some(*executor);

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

    pub fn load_from_path<P>(manifest_path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        serde_json::from_reader(fs::File::open(manifest_path)?)
            .map_err(|e| anyhow!("Problem in loading manifest from path: {e}"))
    }

    pub async fn from_remote(
        world_address: FieldElement,
        rpc_url: Url,
        local_manifest: &Self,
    ) -> Result<Self> {
        let mut manifest = Manifest::default();

        let starknet = JsonRpcClient::new(HttpTransport::new(rpc_url));
        let world_class_hash =
            starknet.get_class_hash_at(&BlockId::Tag(BlockTag::Latest), world_address).await.ok();

        if world_class_hash.is_none() {
            return Ok(manifest);
        }

        let executor_address = starknet
            .get_storage_at(
                world_address,
                get_storage_var_address("executor", &[])?,
                &BlockId::Tag(BlockTag::Latest),
            )
            .await?;
        let executor_class_hash = starknet
            .get_class_hash_at(&BlockId::Tag(BlockTag::Latest), executor_address)
            .await
            .ok();

        manifest.world = world_class_hash;
        manifest.executor = executor_class_hash;

        // Fetch the components/systems class hash if they are registered in the remote World.
        for (component, system) in iter::zip(&local_manifest.components, &local_manifest.systems) {
            let comp_class_hash = starknet
                .call(
                    &FunctionCall {
                        contract_address: world_address,
                        calldata: vec![cairo_short_string_to_felt(&component.name)?],
                        entry_point_selector: get_selector_from_name("component")?,
                    },
                    &BlockId::Tag(BlockTag::Latest),
                )
                .await?[0];

            let syst_class_hash = starknet
                .call(
                    &FunctionCall {
                        contract_address: world_address,
                        calldata: vec![cairo_short_string_to_felt(
                            // because the name returns by the `name` method of
                            // a system contract is without the 'System' suffix
                            system.name.strip_suffix("System").unwrap_or(&system.name),
                        )?],
                        entry_point_selector: get_selector_from_name("system")?,
                    },
                    &BlockId::Tag(BlockTag::Latest),
                )
                .await?[0];

            manifest.components.push(Component {
                name: component.name.clone(),
                class_hash: comp_class_hash,
                ..Default::default()
            });
            manifest.systems.push(System {
                name: system.name.clone(),
                class_hash: syst_class_hash,
                ..Default::default()
            });
        }

        Ok(manifest)
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
        for name in &aux_data.components {
            if let Ok(Some(ModuleItemId::Struct(struct_id))) =
                db.module_item_by_name(module_id, name.clone())
            {
                let members = db
                    .struct_members(struct_id)
                    .unwrap()
                    .iter()
                    .map(|(component_name, member)| Member {
                        name: component_name.to_string(),
                        ty: member.ty.format(db),
                    })
                    .collect();

                // It needs the `Component` suffix because we are
                // searching from the compiled contracts.
                let class_hash = compiled_classes
                    .get(format!("{name}Component").as_str())
                    .with_context(|| format!("Contract {name} not found in target."))
                    .unwrap();

                self.components.push(Component {
                    members,
                    name: name.to_string(),
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
    ) -> Result<(), ManifestError> {
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
                    let signature = db
                        .free_function_signature(fn_id)
                        .map_err(|_| ManifestError::CompilationError)?;

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

                    self.systems.push(System {
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

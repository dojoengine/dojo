use ::serde::{Deserialize, Serialize};
use cairo_lang_defs::ids::{ModuleId, ModuleItemId};
use cairo_lang_filesystem::ids::CrateId;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::plugin::DynPluginAuxData;
use smol_str::SmolStr;
use thiserror::Error;

use crate::plugin::DojoAuxData;

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

#[derive(Error, Debug)]
pub enum ManifestError {
    #[error("Generic traits are unsupported.")]
    GenericTraitsUnsupported,
    #[error("Compilation error.")]
    CompilationError,
    #[error("Got unexpected type.")]
    UnexpectedType,
}

/// Component member.
#[derive(Debug, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// Represents a declaration of a component.
#[derive(Debug, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    pub members: Vec<Member>,
}

/// System input ABI.
#[derive(Debug, Serialize, Deserialize)]
pub struct Input {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// System Output ABI.
#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
    #[serde(rename = "type")]
    pub ty: String,
}

/// Represents a declaration of a system.
#[derive(Debug, Serialize, Deserialize)]
pub struct System {
    pub name: SmolStr,
    pub inputs: Vec<Input>,
    pub outputs: Vec<Output>,
    pub dependencies: Vec<String>,
}

/// Represents a declaration of a system.
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub components: Vec<Component>,
    pub systems: Vec<System>,
}

impl Manifest {
    pub fn new(db: &dyn SemanticGroup, crate_ids: &[CrateId]) -> Self {
        let mut manifest = Manifest { components: vec![], systems: vec![] };
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

                    manifest.find_components(db, aux_data, *module_id);
                    manifest.find_systems(db, aux_data, *module_id).unwrap();
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
    ) {
        for name in &aux_data.components {
            let structs = db.module_structs_ids(module_id);
        
            let component_struct = structs.unwrap()[0];

            let members = db
                .struct_members(component_struct)
                .unwrap()
                .iter()
                .map(|(component_name, member)| Member {
                    name: component_name.to_string(),
                    ty: member.ty.format(db),
                })
                .collect();
        
        
            self.components.push(Component { name: name.to_string(), members });
        }
    }

    fn find_systems(
        &mut self,
        db: &dyn SemanticGroup,
        aux_data: &DojoAuxData,
        module_id: ModuleId,
    ) -> Result<(), ManifestError> {
        for name in &aux_data.systems {
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
                    self.systems.push(System {
                        name: name.clone(),
                        inputs,
                        outputs,
                        dependencies: vec![],
                    });
                }
            } else {
                panic!("System `{name}` was not found.");
            }
        }

        Ok(())
    }
}

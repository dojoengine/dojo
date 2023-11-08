use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::iter::zip;
use std::ops::{Deref, DerefMut};

use anyhow::{anyhow, Context, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_defs::db::DefsGroup;
use cairo_lang_defs::ids::{ModuleId, ModuleItemId};
use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_filesystem::ids::{CrateId, CrateLongId};
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_starknet::abi;
use cairo_lang_starknet::contract::{find_contracts, ContractDeclaration};
use cairo_lang_starknet::contract_class::{compile_prepared_db, ContractClass};
use cairo_lang_starknet::plugin::aux_data::StarkNetContractAuxData;
use cairo_lang_utils::UpcastMut;
use convert_case::{Case, Casing};
use dojo_world::manifest::{
    Class, ComputedValueEntrypoint, Contract, BASE_CONTRACT_NAME, EXECUTOR_CONTRACT_NAME,
    WORLD_CONTRACT_NAME,
};
use itertools::Itertools;
use scarb::compiler::helpers::{build_compiler_config, collect_main_crate_ids};
use scarb::compiler::{CompilationUnit, Compiler};
use scarb::core::{PackageName, TargetKind, Workspace};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::FieldElement;
use tracing::{debug, trace, trace_span};

use crate::inline_macros::utils::{SYSTEM_READS, SYSTEM_WRITES};
use crate::plugin::{ComputedValuesAuxData, DojoAuxData};
use crate::semantics::utils::find_module_rw;

const CAIRO_PATH_SEPARATOR: &str = "::";

#[cfg(test)]
#[path = "compiler_test.rs"]
mod test;

pub struct DojoCompiler;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Props {
    pub build_external_contracts: Option<Vec<ContractSelector>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractSelector(String);

impl ContractSelector {
    fn package(&self) -> PackageName {
        let parts = self.0.split_once(CAIRO_PATH_SEPARATOR).unwrap_or((self.0.as_str(), ""));
        PackageName::new(parts.0)
    }

    fn full_path(&self) -> String {
        self.0.clone()
    }
}

impl Compiler for DojoCompiler {
    fn target_kind(&self) -> TargetKind {
        TargetKind::new("dojo")
    }

    fn compile(
        &self,
        unit: CompilationUnit,
        db: &mut RootDatabase,
        ws: &Workspace<'_>,
    ) -> Result<()> {
        let props: Props = unit.target().props()?;
        let target_dir = unit.target_dir(ws);
        let compiler_config = build_compiler_config(&unit, ws);

        let mut main_crate_ids = collect_main_crate_ids(&unit, db);
        let core_crate_ids: Vec<CrateId> = collect_core_crate_ids(db);
        main_crate_ids.extend(core_crate_ids);

        let contracts = find_project_contracts(
            db.upcast_mut(),
            main_crate_ids.clone(),
            props.build_external_contracts,
        )?;

        let contract_paths = contracts
            .iter()
            .map(|decl| decl.module_id().full_path(db.upcast_mut()))
            .collect::<Vec<_>>();
        trace!(contracts = ?contract_paths);

        let contracts = contracts.iter().collect::<Vec<_>>();

        let classes = {
            let _ = trace_span!("compile_starknet").enter();
            compile_prepared_db(db, &contracts, compiler_config)?
        };

        // (contract name, class hash)
        let mut compiled_classes: HashMap<SmolStr, (FieldElement, Option<abi::Contract>)> =
            HashMap::new();

        for (decl, class) in zip(contracts, classes) {
            let target_name = &unit.target().name;
            let contract_name = decl.submodule_id.name(db.upcast_mut());
            let file_name = format!("{target_name}-{contract_name}.json");

            let mut file = target_dir.open_rw(file_name.clone(), "output file", ws.config())?;
            serde_json::to_writer_pretty(file.deref_mut(), &class)
                .with_context(|| format!("failed to serialize contract: {contract_name}"))?;

            let class_hash = compute_class_hash_of_contract_class(&class).with_context(|| {
                format!("problem computing class hash for contract `{contract_name}`")
            })?;
            compiled_classes.insert(contract_name, (class_hash, class.abi));
        }

        let mut manifest = target_dir
            .open_ro("manifest.json", "output file", ws.config())
            .map(|file| dojo_world::manifest::Manifest::try_from(file.deref()).unwrap_or_default())
            .unwrap_or_default();

        update_manifest(&mut manifest, db, &main_crate_ids, compiled_classes)?;

        manifest.write_to_path(
            target_dir.open_rw("manifest.json", "output file", ws.config())?.path(),
        )?;

        Ok(())
    }
}

fn compute_class_hash_of_contract_class(class: &ContractClass) -> Result<FieldElement> {
    let class_str = serde_json::to_string(&class)?;
    let sierra_class = serde_json::from_str::<SierraClass>(&class_str)
        .map_err(|e| anyhow!("error parsing Sierra class: {e}"))?;
    sierra_class.class_hash().map_err(|e| anyhow!("problem hashing sierra contract: {e}"))
}

fn find_project_contracts(
    mut db: &dyn SemanticGroup,
    main_crate_ids: Vec<CrateId>,
    external_contracts: Option<Vec<ContractSelector>>,
) -> Result<Vec<ContractDeclaration>> {
    let internal_contracts = {
        let _ = trace_span!("find_internal_contracts").enter();
        find_contracts(db, &main_crate_ids)
    };

    let external_contracts = if let Some(external_contracts) = external_contracts {
        let _ = trace_span!("find_external_contracts").enter();
        debug!("external contracts selectors: {:?}", external_contracts);

        let crate_ids = external_contracts
            .iter()
            .map(|selector| selector.package().into())
            .unique()
            .map(|package_name: SmolStr| {
                db.upcast_mut().intern_crate(CrateLongId::Real(package_name))
            })
            .collect::<Vec<_>>();
        find_contracts(db, crate_ids.as_ref())
            .into_iter()
            .filter(|decl| {
                external_contracts.iter().any(|selector| {
                    let contract_path = decl.module_id().full_path(db.upcast());
                    contract_path == selector.full_path()
                })
            })
            .collect::<Vec<ContractDeclaration>>()
    } else {
        debug!("no external contracts selected");
        Vec::new()
    };

    Ok(internal_contracts.into_iter().chain(external_contracts).collect())
}

pub fn collect_core_crate_ids(db: &RootDatabase) -> Vec<CrateId> {
    [
        ContractSelector("dojo::base::base".to_string()),
        ContractSelector("dojo::executor::executor".to_string()),
        ContractSelector("dojo::world::world".to_string()),
    ]
    .iter()
    .map(|selector| selector.package().into())
    .unique()
    .map(|package_name: SmolStr| db.intern_crate(CrateLongId::Real(package_name)))
    .collect::<Vec<_>>()
}

pub fn collect_external_crate_ids(
    db: &RootDatabase,
    external_contracts: Vec<ContractSelector>,
) -> Vec<CrateId> {
    external_contracts
        .iter()
        .map(|selector| selector.package().into())
        .unique()
        .map(|package_name: SmolStr| db.intern_crate(CrateLongId::Real(package_name)))
        .collect::<Vec<_>>()
}

fn update_manifest(
    manifest: &mut dojo_world::manifest::Manifest,
    db: &RootDatabase,
    crate_ids: &[CrateId],
    compiled_artifacts: HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
) -> anyhow::Result<()> {
    fn get_compiled_artifact_from_map<'a>(
        artifacts: &'a HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
        artifact_name: &str,
    ) -> anyhow::Result<&'a (FieldElement, Option<abi::Contract>)> {
        artifacts.get(artifact_name).context(format!(
            "Contract `{artifact_name}` not found. Did you include `dojo` as a dependency?",
        ))
    }

    let world = {
        let (hash, abi) = get_compiled_artifact_from_map(&compiled_artifacts, WORLD_CONTRACT_NAME)?;
        Contract {
            name: WORLD_CONTRACT_NAME.into(),
            abi: abi.clone(),
            class_hash: *hash,
            ..Default::default()
        }
    };

    let executor = {
        let (hash, abi) =
            get_compiled_artifact_from_map(&compiled_artifacts, EXECUTOR_CONTRACT_NAME)?;
        Contract {
            name: EXECUTOR_CONTRACT_NAME.into(),
            abi: abi.clone(),
            class_hash: *hash,
            ..Default::default()
        }
    };

    let base = {
        let (hash, abi) = get_compiled_artifact_from_map(&compiled_artifacts, BASE_CONTRACT_NAME)?;
        Class { name: BASE_CONTRACT_NAME.into(), abi: abi.clone(), class_hash: *hash }
    };

    let mut models = BTreeMap::new();
    let mut contracts = BTreeMap::new();
    let mut computed = BTreeMap::new();

    for crate_id in crate_ids {
        for module_id in db.crate_modules(*crate_id).as_ref() {
            let file_infos = db.module_generated_file_infos(*module_id).unwrap_or_default();
            for aux_data in file_infos
                .iter()
                .skip(1)
                .filter_map(|info| info.as_ref().map(|i| &i.aux_data))
                .filter_map(|aux_data| aux_data.as_ref().map(|aux_data| aux_data.0.as_any()))
            {
                if let Some(aux_data) = aux_data.downcast_ref::<StarkNetContractAuxData>() {
                    contracts.extend(get_dojo_contract_artifacts(
                        db,
                        module_id,
                        aux_data,
                        &compiled_artifacts,
                    )?);
                }
                if let Some(aux_data) = aux_data.downcast_ref::<ComputedValuesAuxData>() {
                    get_dojo_computed_values(db, module_id, aux_data, &mut computed);
                }

                if let Some(dojo_aux_data) = aux_data.downcast_ref::<DojoAuxData>() {
                    models.extend(get_dojo_model_artifacts(
                        db,
                        dojo_aux_data,
                        *module_id,
                        &compiled_artifacts,
                    )?);
                }
            }
        }
    }

    computed.into_iter().for_each(|(contract, computed_value_entrypoint)| {
        let contract_data =
            contracts.get_mut(&contract).expect("Error: Computed value contract doesn't exist.");
        contract_data.computed = computed_value_entrypoint;
    });

    for model in &models {
        contracts.remove(model.0.to_case(Case::Snake).as_str());
    }

    do_update_manifest(manifest, world, executor, base, models, contracts)?;

    Ok(())
}

/// Finds the inline modules annotated as models in the given crate_ids and
/// returns the corresponding Models.
fn get_dojo_model_artifacts(
    db: &dyn SemanticGroup,
    aux_data: &DojoAuxData,
    module_id: ModuleId,
    compiled_classes: &HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
) -> anyhow::Result<HashMap<String, dojo_world::manifest::Model>> {
    let mut models = HashMap::with_capacity(aux_data.models.len());

    for model in &aux_data.models {
        if let Ok(Some(ModuleItemId::Struct(_))) =
            db.module_item_by_name(module_id, model.name.clone().into())
        {
            let model_contract_name = model.name.to_case(Case::Snake);

            let (class_hash, abi) = compiled_classes
                .get(model_contract_name.as_str())
                .cloned()
                .ok_or(anyhow!("Model {} not found in target.", model.name))?;

            models.insert(
                model.name.clone(),
                dojo_world::manifest::Model {
                    abi,
                    class_hash,
                    name: model.name.clone(),
                    members: model.members.clone(),
                },
            );
        }
    }

    Ok(models)
}

fn get_dojo_computed_values(
    db: &RootDatabase,
    module_id: &ModuleId,
    aux_data: &ComputedValuesAuxData,
    computed_values: &mut BTreeMap<SmolStr, Vec<ComputedValueEntrypoint>>,
) {
    if let ModuleId::Submodule(submod_id) = module_id {
        let contract = submod_id.name(db);
        if !computed_values.contains_key(&contract) {
            computed_values.insert(contract.clone(), vec![]);
        }
        let computed_vals = computed_values.get_mut(&contract).unwrap();
        computed_vals.push(ComputedValueEntrypoint {
            contract,
            entrypoint: aux_data.entrypoint.clone(),
            model: aux_data.model.clone(),
        })
    }
}

fn get_dojo_contract_artifacts(
    db: &RootDatabase,
    module_id: &ModuleId,
    aux_data: &StarkNetContractAuxData,
    compiled_classes: &HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
) -> anyhow::Result<HashMap<SmolStr, Contract>> {
    aux_data
        .contracts
        .iter()
        .filter(|name| !matches!(name.as_ref(), "world" | "executor" | "base"))
        .map(|name| {
            let module_name = module_id.full_path(db);
            let module_last_name = module_name.split("::").last().unwrap();

            let reads = match SYSTEM_READS.lock().unwrap().get(module_last_name) {
                Some(models) => {
                    models.clone().into_iter().collect::<BTreeSet<_>>().into_iter().collect()
                }
                None => vec![],
            };

            let write_entries = SYSTEM_WRITES.lock().unwrap();
            let writes = match write_entries.get(module_last_name) {
                Some(write_ops) => find_module_rw(db, module_id, write_ops),
                None => vec![],
            };

            let (class_hash, abi) = compiled_classes
                .get(name)
                .cloned()
                .ok_or(anyhow!("Contract {name} not found in target."))?;

            Ok((
                name.clone(),
                Contract {
                    name: name.clone(),
                    class_hash,
                    abi,
                    writes,
                    reads,
                    ..Default::default()
                },
            ))
        })
        .collect::<anyhow::Result<_>>()
}

fn do_update_manifest(
    current_manifest: &mut dojo_world::manifest::Manifest,
    world: dojo_world::manifest::Contract,
    executor: dojo_world::manifest::Contract,
    base: dojo_world::manifest::Class,
    models: BTreeMap<String, dojo_world::manifest::Model>,
    contracts: BTreeMap<SmolStr, dojo_world::manifest::Contract>,
) -> anyhow::Result<()> {
    if current_manifest.world.class_hash != world.class_hash {
        current_manifest.world = world;
    }

    if current_manifest.executor.class_hash != executor.class_hash {
        current_manifest.executor = executor;
    }

    if current_manifest.base.class_hash != base.class_hash {
        current_manifest.base = base;
    }

    let mut contracts_to_add = vec![];
    for (name, mut contract) in contracts {
        if let Some(existing_contract) =
            current_manifest.contracts.iter_mut().find(|c| c.name == name)
        {
            contract.address = existing_contract.address;
        }
        contracts_to_add.push(contract);
    }

    current_manifest.contracts = contracts_to_add;
    current_manifest.models = models.into_values().collect();

    Ok(())
}

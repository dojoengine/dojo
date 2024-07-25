use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::Write;
use std::iter::zip;
use std::ops::DerefMut;

use anyhow::{anyhow, Context, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_defs::db::DefsGroup;
use cairo_lang_defs::ids::{ModuleId, ModuleItemId, TopLevelLanguageElementId};
use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_filesystem::ids::{CrateId, CrateLongId};
use cairo_lang_formatter::format_string;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_starknet::compile::compile_prepared_db;
use cairo_lang_starknet::contract::{find_contracts, ContractDeclaration};
use cairo_lang_starknet::plugin::aux_data::StarkNetContractAuxData;
use cairo_lang_starknet_classes::abi;
use cairo_lang_starknet_classes::contract_class::ContractClass;
use cairo_lang_utils::UpcastMut;
use camino::Utf8PathBuf;
use convert_case::{Case, Casing};
use dojo_world::contracts::naming;
use dojo_world::manifest::{
    AbiFormat, Class, ComputedValueEntrypoint, DojoContract, DojoModel, Manifest, ManifestMethods,
    ABIS_DIR, BASE_CONTRACT_TAG, BASE_DIR, BASE_QUALIFIED_PATH, CONTRACTS_DIR, MANIFESTS_DIR,
    MODELS_DIR, WORLD_CONTRACT_TAG, WORLD_QUALIFIED_PATH,
};
use dojo_world::metadata::get_namespace_config_from_ws;
use itertools::Itertools;
use scarb::compiler::helpers::{build_compiler_config, collect_main_crate_ids};
use scarb::compiler::{CairoCompilationUnit, CompilationUnitAttributes, Compiler};
use scarb::core::{PackageName, TargetKind, Workspace};
use scarb::flock::Filesystem;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::Felt;
use tracing::{debug, trace, trace_span};

use crate::inline_macros::utils::{SYSTEM_READS, SYSTEM_WRITES};
use crate::plugin::{ComputedValuesAuxData, DojoAuxData, Model};
use crate::semantics::utils::find_module_rw;

const CAIRO_PATH_SEPARATOR: &str = "::";

pub(crate) const LOG_TARGET: &str = "dojo_lang::compiler";

#[cfg(test)]
#[path = "compiler_test.rs"]
mod test;

#[derive(Debug)]
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

    /// Returns the path with the model name in snake case.
    /// This is used to match the output of the `compile()` function and Dojo plugin naming for
    /// models contracts.
    fn path_with_model_snake_case(&self) -> String {
        let (path, last_segment) =
            self.0.rsplit_once(CAIRO_PATH_SEPARATOR).unwrap_or(("", &self.0));

        // We don't want to snake case the whole path because some of names like `erc20`
        // will be changed to `erc_20`, and leading to invalid paths.
        // The model name has to be snaked case as it's how the Dojo plugin names the Model's
        // contract.
        format!("{}{}{}", path, CAIRO_PATH_SEPARATOR, last_segment.to_case(Case::Snake))
    }
}

impl Compiler for DojoCompiler {
    fn target_kind(&self) -> TargetKind {
        TargetKind::new("dojo")
    }

    fn compile(
        &self,
        unit: CairoCompilationUnit,
        db: &mut RootDatabase,
        ws: &Workspace<'_>,
    ) -> Result<()> {
        let props: Props = unit.main_component().target_props()?;
        let target_dir = unit.target_dir(ws);

        // TODO: if we want to output the manifests at the package level,
        // we must iterate on the ws members, to find the location of the
        // sole package with the `dojo` target.
        // In this case, we can use this path to output the manifests.

        let compiler_config = build_compiler_config(&unit, ws);

        trace!(target: LOG_TARGET, unit = %unit.name(), ?props, "Compiling unit dojo compiler.");

        let mut main_crate_ids = collect_main_crate_ids(&unit, db);
        let core_crate_ids: Vec<CrateId> = collect_core_crate_ids(db);
        main_crate_ids.extend(core_crate_ids);

        let contracts = find_project_contracts(
            db.upcast_mut(),
            main_crate_ids.clone(),
            props.build_external_contracts.clone(),
        )?;

        let contract_paths = contracts
            .iter()
            .map(|decl| decl.module_id().full_path(db.upcast_mut()))
            .collect::<Vec<_>>();
        trace!(target: LOG_TARGET, contracts = ?contract_paths);

        let contracts = contracts.iter().collect::<Vec<_>>();

        let classes = {
            let _ = trace_span!("compile_starknet").enter();
            compile_prepared_db(db, &contracts, compiler_config)?
        };

        let mut compiled_classes: HashMap<String, (Felt, ContractClass)> = HashMap::new();

        for (decl, class) in zip(contracts, classes) {
            // note that the qualified path is in snake case while
            // the `full_path()` method of StructId uses the original struct name case.
            // (see in `get_dojo_model_artifacts`)
            let qualified_path = decl.module_id().full_path(db.upcast_mut());

            let class_hash = compute_class_hash_of_contract_class(&class).with_context(|| {
                format!("problem computing class hash for contract `{}`", qualified_path.clone())
            })?;

            compiled_classes.insert(qualified_path, (class_hash, class));
        }

        update_files(
            db,
            ws,
            &target_dir,
            &main_crate_ids,
            compiled_classes,
            props.build_external_contracts,
        )?;
        Ok(())
    }
}

fn compute_class_hash_of_contract_class(class: &ContractClass) -> Result<Felt> {
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
        debug!(target: LOG_TARGET, external_contracts = ?external_contracts, "External contracts selectors.");

        let crate_ids = external_contracts
            .iter()
            .map(|selector| selector.package().into())
            .unique()
            .map(|package_name: SmolStr| {
                debug!(target: LOG_TARGET, %package_name, "Looking for internal crates.");
                db.upcast_mut().intern_crate(CrateLongId::Real(package_name))
            })
            .collect::<Vec<_>>();

        find_contracts(db, crate_ids.as_ref())
            .into_iter()
            .filter(|decl| {
                let contract_path = decl.module_id().full_path(db.upcast());
                external_contracts
                    .iter()
                    .any(|selector| contract_path == selector.path_with_model_snake_case())
            })
            .collect::<Vec<ContractDeclaration>>()
    } else {
        debug!(target: LOG_TARGET, "No external contracts selected.");
        Vec::new()
    };

    Ok(internal_contracts.into_iter().chain(external_contracts).collect())
}

pub fn collect_core_crate_ids(db: &RootDatabase) -> Vec<CrateId> {
    [
        ContractSelector(BASE_QUALIFIED_PATH.to_string()),
        ContractSelector(WORLD_QUALIFIED_PATH.to_string()),
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

fn update_files(
    db: &RootDatabase,
    ws: &Workspace<'_>,
    target_dir: &Filesystem,
    crate_ids: &[CrateId],
    compiled_artifacts: HashMap<String, (Felt, ContractClass)>,
    external_contracts: Option<Vec<ContractSelector>>,
) -> anyhow::Result<()> {
    let namespace_config = get_namespace_config_from_ws(ws)?;

    let profile_name =
        ws.current_profile().expect("Scarb profile expected to be defined.").to_string();
    let relative_manifest_dir = Utf8PathBuf::new().join(MANIFESTS_DIR).join(profile_name);

    // relative path to manifests and abi
    let base_manifests_dir = Utf8PathBuf::new().join(relative_manifest_dir).join(BASE_DIR);
    let base_abis_dir = Utf8PathBuf::new().join(&base_manifests_dir).join(ABIS_DIR);

    let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();

    fn get_compiled_artifact_from_map<'a>(
        artifacts: &'a HashMap<String, (Felt, ContractClass)>,
        qualified_artifact_path: &str,
    ) -> anyhow::Result<&'a (Felt, ContractClass)> {
        artifacts.get(qualified_artifact_path).context(format!(
            "Contract `{qualified_artifact_path}` not found. Did you include `dojo` as a \
             dependency?",
        ))
    }

    let mut crate_ids = crate_ids.to_vec();

    // World and base contracts from Dojo core.
    for (qualified_path, tag) in
        [(WORLD_QUALIFIED_PATH, WORLD_CONTRACT_TAG), (BASE_QUALIFIED_PATH, BASE_CONTRACT_TAG)]
    {
        let (hash, class) = get_compiled_artifact_from_map(&compiled_artifacts, qualified_path)?;
        let filename = naming::get_filename_from_tag(tag);
        write_manifest_and_abi(
            &base_manifests_dir,
            &base_abis_dir,
            &manifest_dir,
            &mut Manifest::new(
                // abi path will be written by `write_manifest`
                Class {
                    class_hash: *hash,
                    abi: None,
                    original_class_hash: *hash,
                    tag: tag.to_string(),
                },
                filename.clone(),
            ),
            &class.abi,
        )?;
        save_json_artifact_file(ws, target_dir, class, &filename, tag)?;
    }

    let mut models = BTreeMap::new();
    let mut contracts = BTreeMap::new();
    let mut computed = BTreeMap::new();

    if let Some(external_contracts) = external_contracts {
        let external_crate_ids = collect_external_crate_ids(db, external_contracts);
        crate_ids.extend(external_crate_ids);
    }

    for crate_id in crate_ids {
        for module_id in db.crate_modules(crate_id).as_ref() {
            let file_infos =
                db.module_generated_file_infos(*module_id).unwrap_or(std::sync::Arc::new([]));
            for aux_data in file_infos
                .iter()
                .skip(1)
                .filter_map(|info| info.as_ref().map(|i| &i.aux_data))
                .filter_map(|aux_data| aux_data.as_ref().map(|aux_data| aux_data.0.as_any()))
            {
                if let Some(aux_data) = aux_data.downcast_ref::<ComputedValuesAuxData>() {
                    get_dojo_computed_values(db, module_id, aux_data, &mut computed);
                } else if let Some(dojo_aux_data) = aux_data.downcast_ref::<DojoAuxData>() {
                    for system in &dojo_aux_data.systems {
                        contracts.extend(get_dojo_contract_artifacts(
                            db,
                            module_id,
                            &naming::get_tag(&system.namespace, &system.name),
                            &compiled_artifacts,
                        )?);
                    }

                    models.extend(get_dojo_model_artifacts(
                        db,
                        &dojo_aux_data.models,
                        *module_id,
                        &compiled_artifacts,
                    )?);
                } else if let Some(aux_data) = aux_data.downcast_ref::<StarkNetContractAuxData>() {
                    contracts.extend(get_dojo_contract_artifacts(
                        db,
                        module_id,
                        &naming::get_tag(&namespace_config.default, &aux_data.contract_name),
                        &compiled_artifacts,
                    )?);
                }
            }
        }
    }

    // `get_dojo_computed_values()` uses the module name as contract name to build the `computed`
    // variable. That means, the namespace of the contract is not taken into account,
    // but should be retrieved from the dojo::contract attribute.
    computed.into_iter().for_each(|(contract, computed_value_entrypoint)| {
        let contract_data = contracts
            .get_mut(&contract.to_string())
            .expect("Error: Computed value contract doesn't exist.");
        contract_data.0.inner.computed = computed_value_entrypoint;
    });

    for model in &models {
        contracts.remove(model.0.as_str());
    }

    let contracts_dir = target_dir.child(CONTRACTS_DIR);
    if !contracts.is_empty() && !contracts_dir.exists() {
        fs::create_dir_all(contracts_dir.path_unchecked())?;
    }

    for (_, (manifest, class, module_id)) in contracts.iter_mut() {
        write_manifest_and_abi(
            &base_manifests_dir.join(CONTRACTS_DIR),
            &base_abis_dir.join(CONTRACTS_DIR),
            &manifest_dir,
            manifest,
            &class.abi,
        )?;

        let filename = naming::get_filename_from_tag(&manifest.inner.tag);
        save_expanded_source_file(
            ws,
            *module_id,
            db,
            &contracts_dir,
            &filename,
            &manifest.inner.tag,
        )?;
        save_json_artifact_file(ws, &contracts_dir, class, &filename, &manifest.inner.tag)?;
    }

    let models_dir = target_dir.child(MODELS_DIR);
    if !models.is_empty() && !models_dir.exists() {
        fs::create_dir_all(models_dir.path_unchecked())?;
    }

    for (_, (manifest, class, module_id)) in models.iter_mut() {
        write_manifest_and_abi(
            &base_manifests_dir.join(MODELS_DIR),
            &base_abis_dir.join(MODELS_DIR),
            &manifest_dir,
            manifest,
            &class.abi,
        )?;

        let filename = naming::get_filename_from_tag(&manifest.inner.tag);
        save_expanded_source_file(ws, *module_id, db, &models_dir, &filename, &manifest.inner.tag)?;
        save_json_artifact_file(ws, &models_dir, class, &filename, &manifest.inner.tag)?;
    }

    Ok(())
}

/// Finds the inline modules annotated as models in the given crate_ids and
/// returns the corresponding Models.
#[allow(clippy::type_complexity)]
fn get_dojo_model_artifacts(
    db: &RootDatabase,
    aux_data: &Vec<Model>,
    module_id: ModuleId,
    compiled_classes: &HashMap<String, (Felt, ContractClass)>,
) -> anyhow::Result<HashMap<String, (Manifest<DojoModel>, ContractClass, ModuleId)>> {
    let mut models = HashMap::with_capacity(aux_data.len());

    for model in aux_data {
        if let Ok(Some(ModuleItemId::Struct(struct_id))) =
            db.module_item_by_name(module_id, model.name.clone().into())
        {
            // Leverages the contract selector function to only snake case the model name and
            // not the full path.
            let contract_selector = ContractSelector(struct_id.full_path(db));
            let qualified_path = contract_selector.path_with_model_snake_case();
            let compiled_class = compiled_classes.get(&qualified_path).cloned();
            let tag = naming::get_tag(&model.namespace, &model.name);

            if let Some((class_hash, class)) = compiled_class {
                models.insert(
                    qualified_path.clone(),
                    (
                        Manifest::new(
                            DojoModel {
                                tag: tag.clone(),
                                class_hash,
                                abi: None,
                                members: model.members.clone(),
                                original_class_hash: class_hash,
                            },
                            naming::get_filename_from_tag(&tag),
                        ),
                        class,
                        module_id,
                    ),
                );
            } else {
                println!("Model {} not found in target.", tag.clone());
            }
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
    if let ModuleId::Submodule(_) = module_id {
        let module_name = module_id.full_path(db);
        let module_name = SmolStr::from(module_name);

        if !computed_values.contains_key(&module_name) {
            computed_values.insert(module_name.clone(), vec![]);
        }
        let computed_vals = computed_values.get_mut(&module_name).unwrap();
        computed_vals.push(ComputedValueEntrypoint {
            contract: module_name,
            entrypoint: aux_data.entrypoint.clone(),
            tag: aux_data.tag.clone(),
        })
    }
}

#[allow(clippy::type_complexity)]
fn get_dojo_contract_artifacts(
    db: &RootDatabase,
    module_id: &ModuleId,
    tag: &str,
    compiled_classes: &HashMap<String, (Felt, ContractClass)>,
) -> anyhow::Result<HashMap<String, (Manifest<DojoContract>, ContractClass, ModuleId)>> {
    let mut result = HashMap::new();

    if !matches!(naming::get_name_from_tag(tag).as_str(), "world" | "resource_metadata" | "base") {
        let qualified_path = module_id.full_path(db).to_string();

        if let Some((class_hash, class)) = compiled_classes.get(&qualified_path) {
            let reads = SYSTEM_READS
                .lock()
                .unwrap()
                .get(&qualified_path as &str)   // should use tag instead of qualified_path
                .map_or_else(Vec::new, |models| {
                    models.clone().into_iter().collect::<BTreeSet<_>>().into_iter().collect()
                });

            let writes = SYSTEM_WRITES
                .lock()
                .unwrap()
                .get(&qualified_path as &str)   // should use tag instead of qualified_path
                .map_or_else(Vec::new, |write_ops| find_module_rw(db, module_id, write_ops));

            let manifest = Manifest::new(
                DojoContract {
                    tag: tag.to_string(),
                    writes,
                    reads,
                    class_hash: *class_hash,
                    original_class_hash: *class_hash,
                    ..Default::default()
                },
                naming::get_filename_from_tag(tag),
            );

            result.insert(qualified_path.to_string(), (manifest, class.clone(), *module_id));
        }
    }

    Ok(result)
}

fn write_manifest_and_abi<T>(
    relative_manifest_dir: &Utf8PathBuf,
    relative_abis_dir: &Utf8PathBuf,
    manifest_dir: &Utf8PathBuf,
    manifest: &mut Manifest<T>,
    abi: &Option<abi::Contract>,
) -> anyhow::Result<()>
where
    T: Serialize + DeserializeOwned + ManifestMethods,
{
    let relative_manifest_path =
        relative_manifest_dir.join(manifest.manifest_name.clone()).with_extension("toml");
    let relative_abi_path =
        relative_abis_dir.join(manifest.manifest_name.clone()).with_extension("json");

    if abi.is_some() {
        manifest.inner.set_abi(Some(AbiFormat::Path(relative_abi_path.clone())));
    }

    let manifest_toml = toml::to_string_pretty(&manifest)?;
    let abi_json = serde_json::to_string_pretty(&abi)?;

    let full_manifest_path = manifest_dir.join(relative_manifest_path);
    let full_abi_path = manifest_dir.join(&relative_abi_path);

    // Create the directory if it doesn't exist
    if let Some(parent) = full_manifest_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }

    std::fs::write(full_manifest_path.clone(), manifest_toml)
        .unwrap_or_else(|_| panic!("Unable to write manifest file to path: {full_manifest_path}"));

    if abi.is_some() {
        // Create the directory if it doesn't exist
        if let Some(parent) = full_abi_path.parent() {
            if !parent.exists() {
                std::fs::create_dir_all(parent)?;
            }
        }

        std::fs::write(full_abi_path.clone(), abi_json)
            .unwrap_or_else(|_| panic!("Unable to write abi file to path: {full_abi_path}"));
    }
    Ok(())
}

fn save_expanded_source_file(
    ws: &Workspace<'_>,
    module_id: ModuleId,
    db: &RootDatabase,
    contract_dir: &Filesystem,
    contract_basename: &str,
    contract_tag: &str,
) -> anyhow::Result<()> {
    if let Ok(files) = db.module_files(module_id) {
        let contract_name = naming::get_name_from_tag(contract_tag);

        // search among all the module files (real and virtual), the one named with
        // the contract/model name. This is the file containing the Cairo code generated
        // from Dojo plugins.
        let res = files.iter().filter(|f| f.file_name(db).eq(&contract_name)).collect::<Vec<_>>();

        let file_id = if res.is_empty() {
            // if there is no virtual file with the name of the contract/model, just use the main
            // module file
            match db.module_main_file(module_id) {
                Ok(f) => f,
                Err(_) => return Err(anyhow!("failed to get source file: {contract_tag}")),
            }
        } else {
            *res[0]
        };

        if let Some(file_content) = db.file_content(file_id) {
            let src_file_name = format!("{contract_basename}.cairo");

            let mut file =
                contract_dir.open_rw(src_file_name.clone(), "source file", ws.config())?;
            file.write(format_string(db, file_content.to_string()).as_bytes())
                .with_context(|| format!("failed to serialize contract source: {contract_tag}"))?;
        } else {
            return Err(anyhow!("failed to get source file content: {contract_tag}"));
        }
    } else {
        return Err(anyhow!("failed to get source file: {contract_tag}"));
    }

    Ok(())
}

fn save_json_artifact_file(
    ws: &Workspace<'_>,
    contract_dir: &Filesystem,
    contract_class: &ContractClass,
    contract_basename: &str,
    contract_tag: &str,
) -> anyhow::Result<()> {
    let mut file =
        contract_dir.open_rw(format!("{contract_basename}.json"), "class file", ws.config())?;
    serde_json::to_writer_pretty(file.deref_mut(), &contract_class)
        .with_context(|| format!("failed to serialize contract artifact: {contract_tag}"))?;
    Ok(())
}

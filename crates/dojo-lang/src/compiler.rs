use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;
use std::iter::zip;
use std::ops::DerefMut;

use anyhow::{anyhow, Context, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_defs::db::DefsGroup;
use cairo_lang_defs::ids::{ModuleId, ModuleItemId};
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
use camino::{Utf8Path, Utf8PathBuf};
use convert_case::{Case, Casing};
use dojo_world::manifest::{
    AbiFormat, Class, ComputedValueEntrypoint, DojoContract, DojoModel, Manifest, ManifestMethods,
    ABIS_DIR, BASE_CONTRACT_NAME, BASE_DIR, CONTRACTS_DIR, MANIFESTS_DIR, MODELS_DIR,
    WORLD_CONTRACT_NAME,
};
use itertools::Itertools;
use scarb::compiler::helpers::{build_compiler_config, collect_main_crate_ids};
use scarb::compiler::{CairoCompilationUnit, CompilationUnitAttributes, Compiler};
use scarb::core::{PackageName, TargetKind, Workspace};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::FieldElement;
use tracing::{debug, trace, trace_span};

use crate::inline_macros::utils::{SYSTEM_READS, SYSTEM_WRITES};
use crate::plugin::{ComputedValuesAuxData, DojoAuxData};
use crate::semantics::utils::find_module_rw;

const CAIRO_PATH_SEPARATOR: &str = "::";

pub const SOURCES_DIR: &str = "src";

pub(crate) const LOG_TARGET: &str = "dojo_lang::compiler";

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
        unit: CairoCompilationUnit,
        db: &mut RootDatabase,
        ws: &Workspace<'_>,
    ) -> Result<()> {
        let props: Props = unit.main_component().target_props()?;
        let target_dir = unit.target_dir(ws);
        let sources_dir = target_dir.child(Utf8Path::new(SOURCES_DIR));

        let compiler_config = build_compiler_config(&unit, ws);

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

        // (contract name, class hash)
        let mut compiled_classes: HashMap<SmolStr, (FieldElement, Option<abi::Contract>)> =
            HashMap::new();

        for (decl, class) in zip(contracts, classes) {
            let contract_full_path = decl.module_id().full_path(db.upcast_mut());

            // save expanded contract source file
            if let Ok(file_id) = db.module_main_file(decl.module_id()) {
                if let Some(file_content) = db.file_content(file_id) {
                    let src_file_name = format!("{contract_full_path}.cairo").replace("::", "_");

                    let mut file =
                        sources_dir.open_rw(src_file_name.clone(), "source file", ws.config())?;
                    file.write(format_string(db, file_content.to_string()).as_bytes())
                        .with_context(|| {
                            format!("failed to serialize contract source: {contract_full_path}")
                        })?;
                } else {
                    return Err(anyhow!("failed to get source file content: {contract_full_path}"));
                }
            } else {
                return Err(anyhow!("failed to get source file: {contract_full_path}"));
            }

            // save JSON artifact file
            let file_name = format!("{contract_full_path}.json");
            let mut file = target_dir.open_rw(file_name.clone(), "class file", ws.config())?;
            serde_json::to_writer_pretty(file.deref_mut(), &class).with_context(|| {
                format!("failed to serialize contract artifact: {contract_full_path}")
            })?;

            let class_hash = compute_class_hash_of_contract_class(&class).with_context(|| {
                format!("problem computing class hash for contract `{contract_full_path}`")
            })?;
            compiled_classes.insert(contract_full_path.into(), (class_hash, class.abi));
        }

        update_manifest(db, ws, &main_crate_ids, compiled_classes, props.build_external_contracts)?;
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
        debug!(target: LOG_TARGET, external_contracts = ?external_contracts, "External contracts selectors.");

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
        debug!(target: LOG_TARGET, "No external contracts selected.");
        Vec::new()
    };

    Ok(internal_contracts.into_iter().chain(external_contracts).collect())
}

pub fn collect_core_crate_ids(db: &RootDatabase) -> Vec<CrateId> {
    [
        ContractSelector(BASE_CONTRACT_NAME.to_string()),
        ContractSelector(WORLD_CONTRACT_NAME.to_string()),
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
    db: &RootDatabase,
    ws: &Workspace<'_>,
    crate_ids: &[CrateId],
    compiled_artifacts: HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
    external_contracts: Option<Vec<ContractSelector>>,
) -> anyhow::Result<()> {
    let profile_name =
        ws.current_profile().expect("Scarb profile expected to be defined.").to_string();
    let profile_dir = Utf8PathBuf::new().join(MANIFESTS_DIR).join(profile_name);

    let relative_manifests_dir = Utf8PathBuf::new().join(&profile_dir).join(BASE_DIR);
    let relative_abis_dir = Utf8PathBuf::new().join(&profile_dir).join(ABIS_DIR).join(BASE_DIR);
    let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();

    fn get_compiled_artifact_from_map<'a>(
        artifacts: &'a HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
        artifact_name: &str,
    ) -> anyhow::Result<&'a (FieldElement, Option<abi::Contract>)> {
        artifacts.get(artifact_name).context(format!(
            "Contract `{artifact_name}` not found. Did you include `dojo` as a dependency?",
        ))
    }

    let mut crate_ids = crate_ids.to_vec();

    let (hash, abi) = get_compiled_artifact_from_map(&compiled_artifacts, WORLD_CONTRACT_NAME)?;
    write_manifest_and_abi(
        &relative_manifests_dir,
        &relative_abis_dir,
        &manifest_dir,
        &mut Manifest::new(
            // abi path will be written by `write_manifest`
            Class { class_hash: *hash, abi: None, original_class_hash: *hash },
            WORLD_CONTRACT_NAME.into(),
        ),
        abi,
    )?;

    let (hash, _) = get_compiled_artifact_from_map(&compiled_artifacts, BASE_CONTRACT_NAME)?;
    write_manifest_and_abi(
        &relative_manifests_dir,
        &relative_abis_dir,
        &manifest_dir,
        &mut Manifest::new(
            Class { class_hash: *hash, abi: None, original_class_hash: *hash },
            BASE_CONTRACT_NAME.into(),
        ),
        &None,
    )?;

    let mut models = BTreeMap::new();
    let mut contracts = BTreeMap::new();
    let mut computed = BTreeMap::new();

    if let Some(external_contracts) = external_contracts {
        let external_crate_ids = collect_external_crate_ids(db, external_contracts);
        crate_ids.extend(external_crate_ids);
    }

    for crate_id in crate_ids {
        for module_id in db.crate_modules(crate_id).as_ref() {
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
        contract_data.0.inner.computed = computed_value_entrypoint;
    });

    for model in &models {
        contracts.remove(model.0.as_str());
    }

    for (_, (manifest, abi)) in contracts.iter_mut() {
        write_manifest_and_abi(
            &relative_manifests_dir.join(CONTRACTS_DIR),
            &relative_abis_dir.join(CONTRACTS_DIR),
            &manifest_dir,
            manifest,
            abi,
        )?;
    }

    for (_, (manifest, abi)) in models.iter_mut() {
        write_manifest_and_abi(
            &relative_manifests_dir.join(MODELS_DIR),
            &relative_abis_dir.join(MODELS_DIR),
            &manifest_dir,
            manifest,
            abi,
        )?;
    }

    Ok(())
}

/// Finds the inline modules annotated as models in the given crate_ids and
/// returns the corresponding Models.
#[allow(clippy::type_complexity)]
fn get_dojo_model_artifacts(
    db: &RootDatabase,
    aux_data: &DojoAuxData,
    module_id: ModuleId,
    compiled_classes: &HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
) -> anyhow::Result<HashMap<String, (Manifest<DojoModel>, Option<abi::Contract>)>> {
    let mut models = HashMap::with_capacity(aux_data.models.len());

    let module_name = module_id.full_path(db);
    let module_name = module_name.as_str();

    for model in &aux_data.models {
        if let Ok(Some(ModuleItemId::Struct(_))) =
            db.module_item_by_name(module_id, model.name.clone().into())
        {
            let model_contract_name = model.name.to_case(Case::Snake);
            let model_full_name = format!("{module_name}::{}", &model_contract_name);

            let compiled_class = compiled_classes.get(model_full_name.as_str()).cloned();

            if let Some((class_hash, abi)) = compiled_class {
                models.insert(
                    model_full_name.clone(),
                    (
                        Manifest::new(
                            DojoModel {
                                members: model.members.clone(),
                                class_hash,
                                original_class_hash: class_hash,
                                abi: None,
                            },
                            model_full_name.into(),
                        ),
                        abi,
                    ),
                );
            } else {
                println!("Model {} not found in target.", model_full_name.clone());
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
            model: aux_data.model.clone(),
        })
    }
}

#[allow(clippy::type_complexity)]
fn get_dojo_contract_artifacts(
    db: &RootDatabase,
    module_id: &ModuleId,
    aux_data: &StarkNetContractAuxData,
    compiled_classes: &HashMap<SmolStr, (FieldElement, Option<abi::Contract>)>,
) -> anyhow::Result<HashMap<SmolStr, (Manifest<DojoContract>, Option<abi::Contract>)>> {
    let contract_name = &aux_data.contract_name;

    let mut result = HashMap::new();

    if !matches!(contract_name.as_ref(), "world" | "resource_metadata" | "base") {
        let module_name: SmolStr = module_id.full_path(db).into();

        if let Some((class_hash, abi)) = compiled_classes.get(&module_name as &str) {
            let reads = SYSTEM_READS
                .lock()
                .unwrap()
                .get(&module_name as &str)
                .map_or_else(Vec::new, |models| {
                    models.clone().into_iter().collect::<BTreeSet<_>>().into_iter().collect()
                });

            let writes = SYSTEM_WRITES
                .lock()
                .unwrap()
                .get(&module_name as &str)
                .map_or_else(Vec::new, |write_ops| find_module_rw(db, module_id, write_ops));

            let manifest = Manifest::new(
                DojoContract {
                    writes,
                    reads,
                    class_hash: *class_hash,
                    original_class_hash: *class_hash,
                    ..Default::default()
                },
                module_name.clone(),
            );

            result.insert(module_name, (manifest, abi.clone()));
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
    let name = manifest.name.to_string().replace("::", "_");

    let relative_manifest_path = relative_manifest_dir.join(name.clone()).with_extension("toml");
    let relative_abi_path = relative_abis_dir.join(name.clone()).with_extension("json");

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

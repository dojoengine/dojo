use std::collections::HashMap;
use std::iter::zip;
use std::ops::DerefMut;

use anyhow::{anyhow, Context, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_filesystem::db::FilesGroup;
use cairo_lang_filesystem::ids::{CrateId, CrateLongId};
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_starknet::contract::{find_contracts, ContractDeclaration};
use cairo_lang_starknet::contract_class::{compile_prepared_db, ContractClass};
use cairo_lang_utils::UpcastMut;
use itertools::Itertools;
use scarb::compiler::helpers::{build_compiler_config, collect_main_crate_ids};
use scarb::compiler::{CompilationUnit, Compiler};
use scarb::core::{PackageName, Workspace};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use starknet::core::types::contract::SierraClass;
use starknet::core::types::FieldElement;
use tracing::{debug, trace, trace_span};

use crate::manifest::Manifest;

const CAIRO_PATH_SEPARATOR: &str = "::";

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
    fn target_kind(&self) -> &str {
        "dojo"
    }

    fn compile(
        &self,
        unit: CompilationUnit,
        db: &mut RootDatabase,
        ws: &Workspace<'_>,
    ) -> Result<()> {
        let props: Props = unit.target().props()?;
        let target_dir = unit.target_dir(ws.config());
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
        let mut compiled_classes: HashMap<SmolStr, FieldElement> = HashMap::new();

        for (decl, class) in zip(contracts, classes) {
            let target_name = &unit.target().name;
            let contract_name = decl.submodule_id.name(db.upcast_mut());
            let file_name = format!("{target_name}-{contract_name}.json");

            let mut file = target_dir.open_rw(file_name.clone(), "output file", ws.config())?;
            serde_json::to_writer_pretty(file.deref_mut(), &class)
                .with_context(|| format!("failed to serialize contract: {contract_name}"))?;

            let class_hash = compute_class_hash_of_contract_class(class).with_context(|| {
                format!("problem computing class hash for contract `{contract_name}`")
            })?;
            compiled_classes.insert(contract_name, class_hash);
        }

        let mut file = target_dir.open_rw("manifest.json", "output file", ws.config())?;
        let manifest = Manifest::new(db, &main_crate_ids, compiled_classes);
        serde_json::to_writer_pretty(file.deref_mut(), &manifest)
            .with_context(|| "failed to serialize manifest")?;

        Ok(())
    }
}

fn compute_class_hash_of_contract_class(class: ContractClass) -> Result<FieldElement> {
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
            .map(|package_name: SmolStr| db.upcast_mut().intern_crate(CrateLongId(package_name)))
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

    Ok(internal_contracts.into_iter().chain(external_contracts.into_iter()).collect())
}

pub fn collect_core_crate_ids(db: &RootDatabase) -> Vec<CrateId> {
    [
        ContractSelector("dojo::executor::executor".to_string()),
        ContractSelector("dojo::world::world".to_string()),
        ContractSelector("dojo::world::library_call".to_string()),
        ContractSelector("dojo::world_factory::world_factory".to_string()),
    ]
    .iter()
    .map(|selector| selector.package().into())
    .unique()
    .map(|package_name: SmolStr| db.intern_crate(CrateLongId(package_name)))
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
        .map(|package_name: SmolStr| db.intern_crate(CrateLongId(package_name)))
        .collect::<Vec<_>>()
}

#[test]
fn test_compiler() {
    use dojo_test_utils::compiler::build_test_config;
    use scarb::ops;

    let config = build_test_config("../../examples/ecs/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));
    let packages = ws.members().map(|p| p.id).collect();
    ops::compile(packages, &ws).unwrap_or_else(|op| panic!("Error compiling: {op:?}"))
}

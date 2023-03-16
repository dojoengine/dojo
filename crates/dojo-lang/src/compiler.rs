use std::iter::zip;
use std::ops::DerefMut;

use anyhow::{ensure, Context, Result};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_starknet::contract::find_contracts;
use cairo_lang_starknet::contract_class::compile_prepared_db;
use cairo_lang_utils::Upcast;
use dojo_project::WorldConfig;
use scarb::compiler::helpers::{
    build_compiler_config, build_project_config, collect_main_crate_ids,
};
use scarb::compiler::{CompilationUnit, Compiler};
use scarb::core::{ExternalTargetKind, Workspace};
use tracing::{trace, trace_span};

use crate::db::DojoRootDatabaseBuilderEx;

pub struct DojoCompiler;

impl Compiler for DojoCompiler {
    fn target_kind(&self) -> &str {
        "dojo"
    }

    fn compile(&self, unit: CompilationUnit, ws: &Workspace<'_>) -> Result<()> {
        let props = unit.target.kind.downcast::<ExternalTargetKind>();
        ensure!(
            props.params.is_empty(),
            "target `{}` does not accept any parameters",
            props.kind_name
        );

        let target_dir = unit.profile.target_dir(ws.config());

        let world_config =
            WorldConfig::from_workspace(ws).unwrap_or_else(|_| WorldConfig::default());

        let mut db = RootDatabase::builder()
            .with_project_config(build_project_config(&unit)?)
            .with_dojo(world_config)
            .build()?;

        let compiler_config = build_compiler_config(&unit, ws);

        let main_crate_ids = collect_main_crate_ids(&unit, &db);

        let contracts = {
            let _ = trace_span!("find_contracts").enter();
            find_contracts(&db, &main_crate_ids)
        };

        trace!(
            contracts = ?contracts
                .iter()
                .map(|decl| decl.module_id().full_path(db.upcast()))
                .collect::<Vec<_>>()
        );

        let contracts = contracts.iter().collect::<Vec<_>>();

        let classes = {
            let _ = trace_span!("compile_starknet").enter();
            compile_prepared_db(&mut db, &contracts, compiler_config)?
        };

        for (decl, class) in zip(contracts, classes) {
            let target_name = &unit.target.name;
            let contract_name = decl.submodule_id.name(db.upcast());
            let mut file = target_dir.open_rw(
                format!("{target_name}_{contract_name}.json"),
                "output file",
                ws.config(),
            )?;
            serde_json::to_writer_pretty(file.deref_mut(), &class)
                .with_context(|| format!("failed to serialize contract: {contract_name}"))?;
        }

        Ok(())
    }
}

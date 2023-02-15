use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use cairo_lang_compiler::db::{RootDatabase, RootDatabaseBuilder};
use cairo_lang_filesystem::db::init_dev_corelib;
use cairo_lang_lowering::db::LoweringGroup;
use cairo_lang_plugins::get_default_plugins;
use cairo_lang_semantic::corelib::get_core_ty_by_name;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::plugin::SemanticPlugin;
use itertools::Itertools;

use crate::plugin::DojoPlugin;

/// Returns a compiler database tuned to Dojo (e.g. Dojo plugin).
pub fn get_dojo_database() -> RootDatabase {
    let mut db_val = RootDatabase::default();
    let db = &mut db_val;

    // Override implicit precedence for compatibility with the Starknet OS.
    db.set_implicit_precedence(Arc::new(
        ["Pedersen", "RangeCheck", "Bitwise", "GasBuiltin", "System"]
            .iter()
            .map(|name| get_core_ty_by_name(db, name.into(), vec![]))
            .collect_vec(),
    ));

    let mut plugins = get_default_plugins();
    plugins.push(Arc::new(DojoPlugin {}));
    db.set_semantic_plugins(plugins);
    db_val
}

pub trait RootDatabaseBuilderDojo {
    fn build_language_server(
        &mut self,
        path: PathBuf,
        plugins: Vec<Arc<dyn SemanticPlugin>>,
    ) -> Result<RootDatabase>;
}

impl RootDatabaseBuilderDojo for RootDatabaseBuilder {
    fn build_language_server(
        &mut self,
        path: PathBuf,
        plugins: Vec<Arc<dyn SemanticPlugin>>,
    ) -> Result<RootDatabase> {
        let mut db = RootDatabase::default();
        init_dev_corelib(&mut db, path);
        db.set_semantic_plugins(plugins);
        Ok(db)
    }
}

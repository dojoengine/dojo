use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use cairo_lang_compiler::db::{RootDatabase, RootDatabaseBuilder};
use cairo_lang_filesystem::db::init_dev_corelib;
use cairo_lang_plugins::get_default_plugins;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::plugin::SemanticPlugin;
use cairo_lang_starknet::plugin::StarkNetPlugin;

use crate::plugin::DojoPlugin;

pub const DOJOLIB_CRATE_NAME: &str = "dojo";

pub trait DojoRootDatabaseBuilderEx {
    fn build_language_server(
        &mut self,
        path: PathBuf,
        plugins: Vec<Arc<dyn SemanticPlugin>>,
    ) -> Result<RootDatabase>;

    /// Tunes a compiler database to Dojo (e.g. Dojo plugin).
    fn with_dojo(&mut self) -> &mut Self;
}

impl DojoRootDatabaseBuilderEx for RootDatabaseBuilder {
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

    fn with_dojo(&mut self) -> &mut Self {
        // Override implicit precedence for compatibility with the Dojo.
        let precedence = ["Pedersen", "RangeCheck", "Bitwise", "EcOp", "GasBuiltin", "System"];

        let mut plugins = get_default_plugins();
        plugins.push(Arc::new(DojoPlugin {}));
        plugins.push(Arc::new(StarkNetPlugin {}));

        self.with_implicit_precedence(&precedence).with_plugins(plugins)
    }
}

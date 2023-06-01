use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use cairo_lang_compiler::db::{RootDatabase, RootDatabaseBuilder};
use cairo_lang_filesystem::db::init_dev_corelib;
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
        self.with_semantic_plugin(Arc::new(DojoPlugin));
        self.with_semantic_plugin(Arc::new(StarkNetPlugin::default()));
        self
    }
}

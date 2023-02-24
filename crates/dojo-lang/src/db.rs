use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use cairo_lang_compiler::db::{RootDatabase, RootDatabaseBuilder};
use cairo_lang_filesystem::db::init_dev_corelib;
use cairo_lang_filesystem::ids::Directory;
use cairo_lang_plugins::get_default_plugins;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::plugin::SemanticPlugin;
use cairo_lang_starknet::plugin::StarkNetPlugin;
use dojo_project::{ProjectConfig, WorldConfig};

use crate::plugin::DojoPlugin;

pub const DOJOLIB_CRATE_NAME: &str = "dojo";

pub trait DojoRootDatabaseBuilderEx {
    fn build_language_server(
        &mut self,
        path: PathBuf,
        plugins: Vec<Arc<dyn SemanticPlugin>>,
    ) -> Result<RootDatabase>;

    /// Tunes a compiler database to Dojo (e.g. Dojo plugin).
    fn with_dojo(&mut self, config: WorldConfig) -> &mut Self;

    fn with_dojo_config(&mut self, config: ProjectConfig) -> &mut Self;
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

    fn with_dojo(&mut self, config: WorldConfig) -> &mut Self {
        // Override implicit precedence for compatibility with the Dojo.
        let precedence = ["Pedersen", "RangeCheck", "Bitwise", "EcOp", "GasBuiltin", "System"];

        let mut plugins = get_default_plugins();
        plugins.push(Arc::new(DojoPlugin { world_config: config }));
        plugins.push(Arc::new(StarkNetPlugin {}));

        self.with_implicit_precedence(&precedence).with_plugins(plugins)
    }

    fn with_dojo_config(&mut self, config: ProjectConfig) -> &mut Self {
        let mut project_config: cairo_lang_project::ProjectConfig = config.clone().into();

        let dir = std::env::var("CAIRO_DOJOLIB_DIR")
            .unwrap_or_else(|e| panic!("Problem getting the dojolib path: {e:?}"));
        project_config.content.crate_roots.insert(DOJOLIB_CRATE_NAME.into(), dir.into());

        let dir = std::env::var("CAIRO_CORELIB_DIR")
            .unwrap_or_else(|e| panic!("Problem getting the corelib path: {e:?}"));
        project_config.corelib = Some(Directory(dir.into()));
        self.with_project_config(project_config);
        self.with_dojo(config.content.world)
    }
}

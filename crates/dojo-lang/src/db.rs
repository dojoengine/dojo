use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use cairo_lang_compiler::db::{RootDatabase, RootDatabaseBuilder};
use cairo_lang_filesystem::db::init_dev_corelib;
use cairo_lang_plugins::get_default_plugins;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::plugin::SemanticPlugin;
use cairo_lang_starknet::plugin::StarkNetPlugin;
use dojo_project::WorldConfig;

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
        plugins.push(Arc::new(DojoPlugin::new(config)));
        plugins.push(Arc::new(StarkNetPlugin {}));

        self.with_implicit_precedence(&precedence).with_plugins(plugins)
    }

    // fn with_dojo_default(&mut self) -> &mut Self {
    //     let core_dir = std::env::var("CAIRO_CORELIB_DIR")
    //         .unwrap_or_else(|e| panic!("Problem getting the corelib path: {e:?}"));
    //     let dojo_dir = std::env::var("DOJOLIB_DIR")
    //         .unwrap_or_else(|e| panic!("Problem getting the dojolib path: {e:?}"));
    //     let config = ProjectConfig {
    //         base_path: "".into(),
    //         content: ProjectConfigContent {
    //             crate_roots: HashMap::from([(DOJOLIB_CRATE_NAME.into(), dojo_dir.into())]),
    //         },
    //         corelib: Some(Directory(core_dir.into())),
    //     };

    //     self.with_project_config(config);
    //     self.with_dojo(WorldConfig::default())
    // }
}

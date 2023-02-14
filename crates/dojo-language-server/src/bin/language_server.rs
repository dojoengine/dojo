use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use cairo_lang_compiler::db::{RootDatabase, RootDatabaseBuilder};
use cairo_lang_filesystem::db::init_dev_corelib;
use cairo_lang_language_server::{Backend, State};
use cairo_lang_plugins::get_default_plugins;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_semantic::plugin::SemanticPlugin;
use cairo_lang_starknet::plugin::StarkNetPlugin;
use dojo_lang::plugin::DojoPlugin;
use tower_lsp::{LspService, Server};

const CORELIB_DIR_NAME: &str = "cairo/corelib";

trait RootDatabaseBuilderDojo {
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

#[tokio::main]
async fn main() {
    #[cfg(feature = "runtime-agnostic")]
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    #[cfg(feature = "runtime-agnostic")]
    let (stdin, stdout) = (stdin.compat(), stdout.compat_write());

    let mut plugins = get_default_plugins();
    plugins.push(Arc::new(DojoPlugin {}));
    plugins.push(Arc::new(StarkNetPlugin {}));

    let mut dir = std::env::current_exe()
        .unwrap_or_else(|e| panic!("Problem getting the executable path: {e:?}"));
    dir.pop();
    dir.pop();
    dir.pop();
    dir.push(CORELIB_DIR_NAME);

    let db = RootDatabase::builder().build_language_server(dir, plugins).unwrap_or_else(|error| {
        panic!("Problem creating language database: {error:?}");
    });

    let (service, socket) = LspService::build(|client| Backend {
        client,
        db_mutex: db.into(),
        state_mutex: State::default().into(),
    })
    .custom_method("vfs/provide", Backend::vfs_provide)
    .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}

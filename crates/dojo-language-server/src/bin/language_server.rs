use std::sync::Arc;

use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_language_server::{Backend, State};
use cairo_lang_plugins::get_default_plugins;
use cairo_lang_starknet::plugin::StarkNetPlugin;
use dojo_lang::db::DojoRootDatabaseBuilderEx;
use dojo_lang::plugin::DojoPlugin;
use dojo_project::WorldConfig;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    #[cfg(feature = "runtime-agnostic")]
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    #[cfg(feature = "runtime-agnostic")]
    let (stdin, stdout) = (stdin.compat(), stdout.compat_write());

    let mut plugins = get_default_plugins();
    plugins.push(Arc::new(DojoPlugin { world_config: WorldConfig::default() }));
    plugins.push(Arc::new(StarkNetPlugin {}));

    let dir = std::env::var("CAIRO_CORELIB_DIR")
        .unwrap_or_else(|e| panic!("Problem getting the corelib path: {e:?}"));

    let db = RootDatabase::builder().build_language_server(dir.into(), plugins).unwrap_or_else(
        |error| {
            panic!("Problem creating language database: {error:?}");
        },
    );

    let (service, socket) = LspService::build(|client| Backend {
        client,
        db_mutex: db.into(),
        state_mutex: State::default().into(),
    })
    .custom_method("vfs/provide", Backend::vfs_provide)
    .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}

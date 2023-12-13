use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_filesystem::cfg::{Cfg, CfgSet};
use cairo_lang_language_server::Backend;
use cairo_lang_starknet::starknet_plugin_suite;
use cairo_lang_test_plugin::test_plugin_suite;
use cairo_lang_utils::logging::init_logging;
use clap::Parser;
use dojo_lang::plugin::dojo_plugin_suite;
use tower_lsp::{LspService, Server};

/// Dojo Language Server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {}

#[tokio::main]
async fn main() {
    let _args = Args::parse();

    init_logging(log::LevelFilter::Warn);

    #[cfg(feature = "runtime-agnostic")]
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    #[cfg(feature = "runtime-agnostic")]
    let (stdin, stdout) = (stdin.compat(), stdout.compat_write());

    let db = RootDatabase::builder()
        .with_cfg(CfgSet::from_iter([Cfg::name("test")]))
        .with_plugin_suite(dojo_plugin_suite())
        .with_plugin_suite(test_plugin_suite())
        .with_plugin_suite(starknet_plugin_suite())
        .build()
        .unwrap_or_else(|error| {
            panic!("Problem creating language database: {error:?}");
        });

    let (service, socket) = LspService::build(|client| Backend::new(client, db))
        .custom_method("vfs/provide", Backend::vfs_provide)
        .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}

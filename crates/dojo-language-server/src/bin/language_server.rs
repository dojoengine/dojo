use std::sync::Arc;

use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_filesystem::cfg::{Cfg, CfgSet};
use cairo_lang_language_server::Backend;
use cairo_lang_starknet::inline_macros::selector::SelectorMacro;
use cairo_lang_starknet::plugin::StarkNetPlugin;
use cairo_lang_test_runner::plugin::TestPlugin;
use cairo_lang_utils::logging::init_logging;
use clap::Parser;
use dojo_lang::inline_macros::emit::EmitMacro;
use dojo_lang::inline_macros::get::GetMacro;
use dojo_lang::inline_macros::set::SetMacro;
use dojo_lang::plugin::BuiltinDojoPlugin;
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
        .with_macro_plugin(Arc::new(TestPlugin::default()))
        .with_macro_plugin(Arc::new(BuiltinDojoPlugin))
        .with_macro_plugin(Arc::new(StarkNetPlugin::default()))
        .with_inline_macro_plugin(EmitMacro::NAME, Arc::new(EmitMacro))
        .with_inline_macro_plugin(GetMacro::NAME, Arc::new(GetMacro))
        .with_inline_macro_plugin(SetMacro::NAME, Arc::new(SetMacro))
        .with_inline_macro_plugin(SelectorMacro::NAME, Arc::new(SelectorMacro))
        .build()
        .unwrap_or_else(|error| {
            panic!("Problem creating language database: {error:?}");
        });

    let (service, socket) = LspService::build(|client| Backend::new(client, db))
        .custom_method("vfs/provide", Backend::vfs_provide)
        .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}

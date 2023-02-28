use std::collections::BTreeMap;
use std::env;
use std::sync::Arc;

use anyhow::Result;
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_filesystem::db::FilesGroupEx;
use cairo_lang_filesystem::ids::{CrateLongId, Directory};
use cairo_lang_language_server::{Backend, State};
use cairo_lang_plugins::get_default_plugins;
use cairo_lang_semantic::db::SemanticGroup;
use cairo_lang_starknet::plugin::StarkNetPlugin;
use dojo_lang::plugin::DojoPlugin;
use dojo_project::WorldConfig;
use scarb::core::{Config, PackageId};
use scarb::metadata::{MetadataOptions, MetadataVersion, PackageMetadata, ProjectMetadata};
use scarb::ops;
use scarb::ui::Verbosity;
use smol_str::SmolStr;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    #[cfg(feature = "runtime-agnostic")]
    use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());
    #[cfg(feature = "runtime-agnostic")]
    let (stdin, stdout) = (stdin.compat(), stdout.compat_write());

    let metadata = read_metadata().unwrap_or_else(|error| {
        panic!("Problem reading metadata: {error:?}");
    });

    let mut plugins = get_default_plugins();
    plugins.push(Arc::new(DojoPlugin { world_config: WorldConfig::default() }));
    plugins.push(Arc::new(StarkNetPlugin {}));

    let mut db =
        RootDatabase::builder().detect_corelib().with_plugins(plugins).build().unwrap_or_else(
            |error| {
                panic!("Problem creating language database: {error:?}");
            },
        );

    update_crate_roots_from_metadata(&mut db, metadata);

    let (service, socket) = LspService::build(|client| Backend {
        client,
        db_mutex: db.into(),
        state_mutex: State::default().into(),
    })
    .custom_method("vfs/provide", Backend::vfs_provide)
    .finish();
    Server::new(stdin, stdout, socket).serve(service).await;
}

fn read_metadata() -> Result<ProjectMetadata> {
    let manifest_path = ops::find_manifest_path(None).unwrap();

    let config = Config::builder(manifest_path)
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .build()
        .unwrap();

    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap();

    let opts = MetadataOptions { version: MetadataVersion::V1, no_deps: false };

    ProjectMetadata::collect(&ws, &opts)
}

fn update_crate_roots_from_metadata(db: &mut dyn SemanticGroup, project_metadata: ProjectMetadata) {
    let packages: BTreeMap<PackageId, PackageMetadata> =
        project_metadata.packages.into_iter().map(|package| (package.id, package)).collect();

    for unit in project_metadata.compilation_units {
        for package_id in unit.components {
            let package_metadata = packages.get(&package_id).unwrap();
            let package_id = SmolStr::from(package_metadata.name.clone());
            let src_path = package_metadata.root.clone().join("src");
            if src_path.exists() {
                let crate_id = db.intern_crate(CrateLongId(package_id));
                let root = Directory(src_path.into());
                db.set_crate_root(crate_id, Some(root));
            };
        }
    }
}

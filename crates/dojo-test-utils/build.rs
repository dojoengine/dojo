#[cfg(feature = "build-examples")]
fn main() {
    use std::env;

    use camino::{Utf8Path, Utf8PathBuf};
    use dojo_lang::compiler::DojoCompiler;
    use dojo_lang::plugin::CairoPluginRepository;
    use dojo_lang::scarb_internal::compile_workspace;
    use scarb::compiler::{CompilerRepository, Profile};
    use scarb::core::{Config, TargetKind};
    use scarb::ops::{CompileOpts, FeaturesOpts, FeaturesSelector};
    use scarb_ui::Verbosity;

    let project_paths = [
        ("../../examples/spawn-and-move", Profile::DEV),
        ("../torii/types-test", Profile::DEV),
        ("../../examples/spawn-and-move", Profile::RELEASE),
    ];

    project_paths.iter().for_each(|(path, profile)| compile(path, profile));

    println!("cargo:rerun-if-changed=../../examples");
    println!("cargo:rerun-if-changed=../torii/types-test");
    println!("cargo:rerun-if-changed=../dojo-lang/src");
    println!("cargo:rerun-if-changed=../../bin/sozo/src");

    fn compile(path: &str, profile: &Profile) {
        let target_path =
            Utf8PathBuf::from_path_buf(format!("{}/target/{}", path, profile).into()).unwrap();
        if target_path.exists() {
            return;
        }

        let mut compilers = CompilerRepository::empty();
        compilers.add(Box::new(DojoCompiler)).unwrap();

        let cairo_plugins = CairoPluginRepository::default();

        let cache_dir = assert_fs::TempDir::new().unwrap();
        let config_dir = assert_fs::TempDir::new().unwrap();

        let path = Utf8PathBuf::from_path_buf(format!("{}/Scarb.toml", path).into()).unwrap();
        let config = Config::builder(path.canonicalize_utf8().unwrap())
            .global_cache_dir_override(Some(Utf8Path::from_path(cache_dir.path()).unwrap()))
            .global_config_dir_override(Some(Utf8Path::from_path(config_dir.path()).unwrap()))
            .ui_verbosity(Verbosity::Verbose)
            .log_filter_directive(env::var_os("SCARB_LOG"))
            .compilers(compilers)
            .profile(profile.to_owned())
            .cairo_plugins(cairo_plugins.into())
            .build()
            .unwrap();

        let features_opts =
            FeaturesOpts { features: FeaturesSelector::AllFeatures, no_default_features: false };
        let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();
        let packages: Vec<scarb::core::PackageId> = ws.members().map(|p| p.id).collect();

        compile_workspace(
            &config,
            CompileOpts {
                include_target_kinds: vec![],
                exclude_target_kinds: vec![TargetKind::TEST],
                include_target_names: vec![],
                features: features_opts,
            },
            packages,
        )
        .unwrap();
    }
}

#[cfg(not(feature = "build-examples"))]
fn main() {}

use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::{env, fs, io};

use assert_fs::TempDir;
use camino::{Utf8Path, Utf8PathBuf};
use dojo_lang::compiler::DojoCompiler;
use dojo_lang::plugin::CairoPluginRepository;
use dojo_lang::scarb_internal::{compile_workspace, CompileInfo};
use scarb::compiler::CompilerRepository;
use scarb::core::{Config, TargetKind};
use scarb::ops;
use scarb::ops::CompileOpts;
use scarb_ui::Verbosity;
use toml::{Table, Value};

/// Copies a project to a new location, excluding the manifests
/// and target directories, build the temporary project and
/// return the temporary project directory.
///
/// # Arguments
///
/// * `source_project_path` - The path to the source project to copy and build at the temporary
///   location.
/// * `do_build` - Whether to build the temporary project. Only use this if you want to build the
///   project again to re-generate all the artifacts. This is a slow operation on the CI (~70s), use
///   it wisely.
pub fn copy_build_project_temp(
    source_project_path: &str,
    do_build: bool,
) -> (Utf8PathBuf, Config, Option<CompileInfo>) {
    let source_project_dir = Utf8PathBuf::from(source_project_path).parent().unwrap().to_path_buf();

    let temp_project_dir = Utf8PathBuf::from(
        assert_fs::TempDir::new().unwrap().to_path_buf().to_string_lossy().to_string(),
    );

    let temp_project_path = temp_project_dir.join("Scarb").with_extension("toml").to_string();

    copy_project_temp(&source_project_dir, &temp_project_dir).unwrap();

    let config = build_test_config_default(&temp_project_path).unwrap();

    let compile_info = if do_build {
        Some(
            compile_workspace(
                &config,
                CompileOpts { include_targets: vec![], exclude_targets: vec![TargetKind::TEST] },
            )
            .unwrap(),
        )
    } else {
        None
    };

    (temp_project_dir, config, compile_info)
}

/// Copies a project to a new location, excluding the manifests and target directories.
///
/// # Arguments
///
/// * `source_dir` - The source directory to copy from.
pub fn copy_project_temp(
    source_dir: &Utf8PathBuf,
    destination_dir: &Utf8PathBuf,
) -> io::Result<()> {
    let ignore_dirs = ["manifests", "target"];

    if !destination_dir.exists() {
        fs::create_dir_all(destination_dir)?;
    }

    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let dir_name = match entry.file_name().into_string() {
                Ok(name) => name,
                Err(_) => continue, // Skip directories/files with non-UTF8 names
            };

            if ignore_dirs.contains(&dir_name.as_str()) {
                continue; // Skip ignored directories
            }

            copy_project_temp(
                &Utf8PathBuf::from_path_buf(path).unwrap(),
                &destination_dir.join(dir_name),
            )?;
        } else {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let dest_path = destination_dir.join(&file_name);

            // Replace in the Scarb.toml the path of dojo crate with the
            // absolute path.
            if file_name == "Scarb.toml" {
                let mut contents = String::new();
                File::open(&path)
                    .and_then(|mut file| file.read_to_string(&mut contents))
                    .unwrap_or_else(|_| panic!("Failed to read {file_name}"));

                let mut table = contents.parse::<Table>().expect("Failed to parse Scab.toml");

                let dojo = table["dependencies"]["dojo"].as_table_mut().unwrap();

                let absolute_path = Value::String(
                    fs::canonicalize(Utf8PathBuf::from(dojo["path"].as_str().unwrap()))
                        .unwrap()
                        .to_string_lossy()
                        .to_string(),
                );

                dojo["path"] = absolute_path;

                File::create(&dest_path)
                    .and_then(|mut file| file.write_all(table.to_string().as_bytes()))
                    .expect("Failed to write to Scab.toml");
            } else {
                fs::copy(path, dest_path)?;
            }
        }
    }

    Ok(())
}

pub fn build_test_config_default(path: &str) -> anyhow::Result<Config> {
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let cairo_plugins = CairoPluginRepository::default();

    let path = Utf8PathBuf::from_path_buf(path.into()).unwrap();
    Config::builder(path.canonicalize_utf8().unwrap())
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .compilers(compilers)
        .cairo_plugins(cairo_plugins.into())
        .build()
}

pub fn build_test_config(path: &str) -> anyhow::Result<Config> {
    build_full_test_config(path, true)
}

pub fn build_full_test_config(path: &str, override_dirs: bool) -> anyhow::Result<Config> {
    let mut compilers = CompilerRepository::empty();
    compilers.add(Box::new(DojoCompiler)).unwrap();

    let cairo_plugins = CairoPluginRepository::default();
    let path = Utf8PathBuf::from_path_buf(path.into()).unwrap();

    if override_dirs {
        let cache_dir = TempDir::new().unwrap();
        let config_dir = TempDir::new().unwrap();
        let target_dir = TempDir::new().unwrap();

        Config::builder(path.canonicalize_utf8().unwrap())
            .global_cache_dir_override(Some(Utf8Path::from_path(cache_dir.path()).unwrap()))
            .global_config_dir_override(Some(Utf8Path::from_path(config_dir.path()).unwrap()))
            .target_dir_override(Some(
                Utf8Path::from_path(target_dir.path()).unwrap().to_path_buf(),
            ))
            .ui_verbosity(Verbosity::Verbose)
            .log_filter_directive(env::var_os("SCARB_LOG"))
            .compilers(compilers)
            .cairo_plugins(cairo_plugins.into())
            .build()
    } else {
        Config::builder(path.canonicalize_utf8().unwrap())
            .ui_verbosity(Verbosity::Verbose)
            .log_filter_directive(env::var_os("SCARB_LOG"))
            .compilers(compilers)
            .cairo_plugins(cairo_plugins.into())
            .build()
    }
}

pub fn corelib() -> PathBuf {
    let config = build_test_config("./src/manifest_test_data/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap();
    let resolve = ops::resolve_workspace(&ws).unwrap();
    let compilation_units = ops::generate_compilation_units(&resolve, &ws).unwrap();
    compilation_units[0]
        .core_package_component()
        .expect("should have component")
        .target
        .source_root()
        .into()
}

#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::Write;

    use assert_fs::TempDir;

    use super::*;

    #[test]
    fn test_copy_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path().join("project");
        let dest_dir = temp_dir.path().join("dest");

        fs::create_dir(&project_dir).unwrap();
        fs::create_dir(&dest_dir).unwrap();

        // Create a file in the project directory
        let file_path = project_dir.join("file.txt");
        let mut file = File::create(file_path).unwrap();
        writeln!(file, "Hello, world!").unwrap();

        // Create a subdirectory with a file in the project directory
        let sub_dir = project_dir.join("subdir");
        fs::create_dir(&sub_dir).unwrap();
        let sub_file_path = sub_dir.join("subfile.txt");
        let mut sub_file = File::create(sub_file_path).unwrap();
        writeln!(sub_file, "Hello, from subdir!").unwrap();

        // Create a subdir that should be ignored
        let ignored_sub_dir = project_dir.join("manifests");
        fs::create_dir(&ignored_sub_dir).unwrap();
        let ignored_sub_file_path = ignored_sub_dir.join("ignored_file.txt");
        let mut ignored_sub_file = File::create(ignored_sub_file_path).unwrap();
        writeln!(ignored_sub_file, "This should be ignored!").unwrap();

        // Perform the copy
        copy_project_temp(
            &Utf8PathBuf::from(&project_dir.to_string_lossy()),
            &Utf8PathBuf::from(&dest_dir.to_string_lossy()),
        )
        .unwrap();

        // Check that the file exists in the destination directory
        let dest_file_path = dest_dir.join("file.txt");
        assert!(dest_file_path.exists());

        // Check that the subdirectory and its file exist in the destination directory
        let dest_sub_dir = dest_dir.join("subdir");
        let dest_sub_file_path = dest_sub_dir.join("subfile.txt");
        let dest_ignored_sub_dir = dest_sub_dir.join("manifests");
        assert!(dest_sub_dir.exists());
        assert!(dest_sub_file_path.exists());
        assert!(!dest_ignored_sub_dir.exists());

        // Clean up
        temp_dir.close().unwrap();
    }
}

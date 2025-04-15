use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::{env, fs, io};

use assert_fs::TempDir;
use camino::{Utf8Path, Utf8PathBuf};
use scarb::compiler::{CompilationUnit, Profile};
use scarb::core::{Config, TargetKind};
use scarb::ops::{self, CompilationUnitsOpts, CompileOpts, FeaturesOpts, FeaturesSelector};
use scarb_ui::Verbosity;
use toml::{Table, Value};

#[derive(Debug)]
pub struct CompilerTestSetup {
    pub dir: Utf8PathBuf,
    pub dojo_core: Utf8PathBuf,
    pub manifests: HashMap<String, Utf8PathBuf>,
}

impl CompilerTestSetup {
    pub fn from_examples(dojo_core: &str, example_path: &str) -> CompilerTestSetup {
        let example_path = Utf8PathBuf::from(example_path);

        let packages: Vec<Utf8PathBuf> =
            vec![example_path.join("spawn-and-move"), example_path.join("game-lib")];

        let package_refs: Vec<&str> = packages.iter().map(|p| p.as_str()).collect();

        Self::from_paths(dojo_core, &package_refs)
    }

    pub fn from_paths(dojo_core: &str, packages: &[&str]) -> CompilerTestSetup {
        let dojo_core = Utf8PathBuf::from(dojo_core);
        let packages: Vec<Utf8PathBuf> = packages.iter().map(Utf8PathBuf::from).collect();

        let tmp_dir = Utf8PathBuf::from(
            assert_fs::TempDir::new().unwrap().to_path_buf().to_string_lossy().to_string(),
        );

        let mut manifests = HashMap::new();

        for package_source in &packages {
            let package_name = package_source.file_name().unwrap();
            let package_tmp = tmp_dir.join(package_name);
            fs::create_dir_all(&package_tmp).unwrap();

            let package_manifest = package_tmp.join("Scarb.toml");

            manifests.insert(package_name.to_string(), package_manifest);

            copy_project_temp(package_source, &package_tmp, &dojo_core, &[]).unwrap();
        }

        CompilerTestSetup { dir: tmp_dir, dojo_core, manifests }
    }

    pub fn build_test_config(&self, package_name: &str, profile: Profile) -> Config {
        let manifest = self.manifests.get(package_name).unwrap();

        build_test_config(manifest.as_ref(), profile)
            .unwrap_or_else(|c| panic!("Error loading config: {c:?}"))
    }
}

/// Copies a directory into a temporary directory.
///
/// # Returns
///
/// A [`Utf8PathBuf`] object pointing to the copied directory.
pub fn copy_tmp_dir(source_dir: &Utf8PathBuf) -> Utf8PathBuf {
    let temp_project_dir = Utf8PathBuf::from(
        assert_fs::TempDir::new().unwrap().to_path_buf().to_string_lossy().to_string(),
    );

    fn copy_dir_recursively(src: &PathBuf, dst: &PathBuf) -> io::Result<()> {
        if src.is_dir() {
            fs::create_dir_all(dst)?;
            for entry in fs::read_dir(src)? {
                let entry = entry?;
                let path = entry.path();
                let dst_path = dst.join(path.file_name().unwrap());
                if path.is_dir() {
                    copy_dir_recursively(&path, &dst_path)?;
                } else {
                    fs::copy(&path, &dst_path)?;
                }
            }
        } else {
            fs::copy(src, dst)?;
        }
        Ok(())
    }

    copy_dir_recursively(&source_dir.to_path_buf().into(), &temp_project_dir.to_path_buf().into())
        .unwrap_or_else(|e| panic!("Failed to copy directory: {}", e));

    temp_project_dir
}

/// Copies a project into a temporary directory and loads a config from the copied project.
///
/// # Returns
///
/// A [`Config`] object loaded from the spawn-and-moves Scarb.toml file.
pub fn copy_tmp_config(
    source_project_dir: &Utf8PathBuf,
    dojo_core_path: &Utf8PathBuf,
    profile: Profile,
) -> Config {
    let temp_project_dir = Utf8PathBuf::from(
        assert_fs::TempDir::new().unwrap().to_path_buf().to_string_lossy().to_string(),
    );

    let temp_project_path = temp_project_dir.join("Scarb").with_extension("toml").to_string();

    // Copy all the files, including manifests. As we will not re-build, mostly only migrate.
    copy_project_temp(source_project_dir, &temp_project_dir, dojo_core_path, &[]).unwrap();

    build_test_config(&temp_project_path, profile)
        .unwrap_or_else(|c| panic!("Error loading config: {c:?}"))
}

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
    dojo_core_path: &str,
    do_build: bool,
    profile: Profile,
) -> (Utf8PathBuf, Config) {
    let source_project_dir = Utf8PathBuf::from(source_project_path).parent().unwrap().to_path_buf();

    let temp_project_dir = Utf8PathBuf::from(
        assert_fs::TempDir::new().unwrap().to_path_buf().to_string_lossy().to_string(),
    );

    let temp_project_path = temp_project_dir.join("Scarb").with_extension("toml").to_string();

    let dojo_core_path = Utf8PathBuf::from(dojo_core_path);
    // we don't ignore `manifests` because `overylays` are required for successful migration
    let ignore_dirs = ["target"];

    copy_project_temp(&source_project_dir, &temp_project_dir, &dojo_core_path, &ignore_dirs)
        .unwrap();

    let config = build_test_config(&temp_project_path, profile).unwrap();

    let features_opts =
        FeaturesOpts { features: FeaturesSelector::AllFeatures, no_default_features: false };
    let ws = scarb::ops::read_workspace(config.manifest_path(), &config).unwrap();

    let packages: Vec<scarb::core::PackageId> = ws.members().map(|p| p.id).collect();

    if do_build {
        scarb::ops::compile(
            packages,
            CompileOpts {
                include_target_names: vec![],
                include_target_kinds: vec![],
                exclude_target_kinds: vec![TargetKind::TEST],
                features: features_opts,
                ignore_cairo_version: true,
            },
            &ws,
        )
        .unwrap();
    }

    (temp_project_dir, config)
}

/// Copies a project to a new location, excluding the manifests and target directories.
///
/// # Arguments
///
/// * `source_dir` - The source directory to copy from.
pub fn copy_project_temp(
    source_dir: &Utf8PathBuf,
    destination_dir: &Utf8PathBuf,
    dojo_core_path: &Utf8PathBuf,
    ignore_dirs: &[&str],
) -> io::Result<()> {
    if !destination_dir.exists() {
        fs::create_dir_all(destination_dir)?;
    }

    let dojo_cairo_test_path =
        Utf8PathBuf::from(dojo_core_path.to_string().replace("core", "core-cairo-test"));

    for entry in fs::read_dir(source_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let dir_name = match entry.file_name().into_string() {
                Ok(name) => name,
                Err(_) => continue, // Skip non UTF8 dirs.
            };

            if ignore_dirs.contains(&dir_name.as_str()) {
                continue;
            }

            copy_project_temp(
                &Utf8PathBuf::from_path_buf(path).unwrap(),
                &destination_dir.join(dir_name),
                dojo_core_path,
                ignore_dirs,
            )?;
        } else {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let dest_path = destination_dir.join(&file_name);

            fs::copy(&path, &dest_path)?;

            // Replace in the Scarb.toml the path of dojo crate with the
            // absolute path.
            if file_name == "Scarb.toml" {
                let mut contents = String::new();
                File::open(&dest_path)
                    .and_then(|mut file| file.read_to_string(&mut contents))
                    .unwrap_or_else(|_| panic!("Failed to read {file_name}"));

                let mut table = contents.parse::<Table>().expect("Failed to parse Scab.toml");

                let dojo = if table.contains_key("workspace") {
                    table["workspace"]["dependencies"]["dojo"].as_table_mut().unwrap()
                } else {
                    table["dependencies"]["dojo"].as_table_mut().unwrap()
                };

                if dojo.contains_key("path") {
                    dojo["path"] = Value::String(
                        fs::canonicalize(dojo_core_path).unwrap().to_string_lossy().to_string(),
                    );

                    fs::write(dest_path.to_path_buf(), table.to_string().as_bytes())
                        .expect("Failed to write to Scab.toml");
                }

                let dojo_cairo_test = if table.contains_key("workspace") {
                    if table["workspace"].as_table().unwrap().contains_key("dev-dependencies") {
                        Some(
                            table["workspace"]["dev-dependencies"]["dojo_cairo_test"]
                                .as_table_mut()
                                .unwrap(),
                        )
                    } else {
                        None
                    }
                } else if table.contains_key("dev-dependencies") {
                    Some(table["dev-dependencies"]["dojo_cairo_test"].as_table_mut().unwrap())
                } else {
                    None
                };

                if let Some(dojo_cairo_test) = dojo_cairo_test {
                    if dojo_cairo_test.contains_key("path") {
                        dojo_cairo_test["path"] = Value::String(
                            fs::canonicalize(dojo_cairo_test_path.clone())
                                .unwrap()
                                .to_string_lossy()
                                .to_string(),
                        );

                        fs::write(dest_path.to_path_buf(), table.to_string().as_bytes())
                            .expect("Failed to write to Scab.toml");
                    }
                }
            }
        }
    }

    Ok(())
}

/// Builds a test config with a temporary cache directory.
///
/// As manifests files are not related to the target_dir, it is recommended
/// to use copy_build_project_temp to copy the project to a temporary location
/// and build the config from there. This ensures safe and non conflicting
/// manipulation of the artifacts and manifests.
///
/// # Arguments
///
/// * `path` - The path to the Scarb.toml file to build the config for.
/// * `profile` - The profile to use for the config.
pub fn build_test_config(path: &str, profile: Profile) -> anyhow::Result<Config> {
    // If the cache_dir is not overridden, we can't run tests in parallel.
    let cache_dir = TempDir::new().unwrap();

    let path = Utf8PathBuf::from_path_buf(path.into()).unwrap();
    println!("path: {:?}", path);
    Config::builder(path.canonicalize_utf8().unwrap())
        .global_cache_dir_override(Some(Utf8Path::from_path(cache_dir.path()).unwrap()))
        .ui_verbosity(Verbosity::Verbose)
        .log_filter_directive(env::var_os("SCARB_LOG"))
        .profile(profile)
        .build()
}

pub fn corelib() -> PathBuf {
    let config =
        build_test_config("./src/manifest_test_data/spawn-and-move/Scarb.toml", Profile::DEV)
            .unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config).unwrap();
    let resolve = ops::resolve_workspace(&ws).unwrap();

    let features_opts =
        FeaturesOpts { features: FeaturesSelector::AllFeatures, no_default_features: false };

    let cu_opts = CompilationUnitsOpts { ignore_cairo_version: false, load_prebuilt_macros: false };

    let compilation_units =
        ops::generate_compilation_units(&resolve, &features_opts, &ws, cu_opts).unwrap();

    if let CompilationUnit::Cairo(unit) = &compilation_units[0] {
        unit.core_package_component().expect("should have component").targets[0]
            .source_root()
            .into()
    } else {
        panic!("should have cairo compilation unit")
    }
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

        let ignore_dirs = ["manifests", "target"];

        copy_project_temp(
            &Utf8PathBuf::from(&project_dir.to_string_lossy()),
            &Utf8PathBuf::from(&dest_dir.to_string_lossy()),
            &Utf8PathBuf::from("../dojo/core"),
            &ignore_dirs,
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

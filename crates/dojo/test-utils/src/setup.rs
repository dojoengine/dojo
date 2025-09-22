use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::{fs, io};

use camino::Utf8PathBuf;
use scarb_interop::Profile;
use scarb_metadata::Metadata;
use scarb_metadata_ext::MetadataDojoExt;
use toml::{Table, Value};

#[derive(Debug)]
pub struct TestSetup {
    pub root_dir: Utf8PathBuf,
    pub dojo_core: Utf8PathBuf,
    pub manifest_paths: HashMap<String, Utf8PathBuf>,
}

impl TestSetup {
    /// Build a test setup from Dojo core and examples pathes.
    pub fn from_examples(dojo_core: &str, example_path: &str) -> TestSetup {
        let example_path = Utf8PathBuf::from(example_path).canonicalize_utf8().unwrap();
        let dojo_core = Utf8PathBuf::from(dojo_core).canonicalize_utf8().unwrap();

        let packages: Vec<Utf8PathBuf> = vec![
            example_path.join("spawn-and-move"),
            example_path.join("game-lib"),
            example_path.join("simple"),
        ];

        Self::from_paths(&dojo_core, &packages)
    }

    pub fn from_paths(dojo_core: &Utf8PathBuf, packages: &[Utf8PathBuf]) -> TestSetup {
        let tmp_dir = Utf8PathBuf::from(
            assert_fs::TempDir::new().unwrap().to_path_buf().to_string_lossy().to_string(),
        );

        let mut manifest_paths = HashMap::new();

        for package_source in packages {
            let package_name = package_source.file_name().unwrap();
            let package_tmp_dir = tmp_dir.join(package_name);
            fs::create_dir_all(&package_tmp_dir).unwrap();

            let package_manifest_path = package_tmp_dir.join("Scarb.toml");

            manifest_paths.insert(package_name.to_string(), package_manifest_path);

            Self::copy_project(package_source, &package_tmp_dir, dojo_core, &["external"]).unwrap();
        }

        TestSetup { root_dir: tmp_dir, dojo_core: dojo_core.clone(), manifest_paths }
    }

    pub fn manifest_path(&self, package_name: &str) -> &Utf8PathBuf {
        self.manifest_paths
            .get(package_name)
            .unwrap_or_else(|| panic!("No manifest for {}", package_name))
    }

    pub fn manifest_dir(&self, package_name: &str) -> Utf8PathBuf {
        self.manifest_path(package_name).parent().unwrap().to_path_buf()
    }

    pub fn load_metadata(&self, package_name: &str, profile: Profile) -> Metadata {
        let manifest_path = self.manifest_paths.get(package_name).unwrap();
        Metadata::load(manifest_path, profile.as_str(), false).unwrap()
    }

    fn update_crates_in_manifest_file(manifest_path: &Utf8PathBuf, dojo_core_path: &Utf8PathBuf) {
        fn update_dep_path(dep: &mut toml::map::Map<String, Value>, new_dep_path: &Utf8PathBuf) {
            if dep.contains_key("path") {
                dep["path"] = Value::String(
                    fs::canonicalize(new_dep_path.clone()).unwrap().to_string_lossy().to_string(),
                );
            }
        }
        fn update_dependency(
            manifest_path: &Utf8PathBuf,
            table: &mut toml::map::Map<String, Value>,
            dep_name: &str,
            new_dep_path: &Utf8PathBuf,
        ) {
            let dep = if table.contains_key("workspace") {
                table["workspace"]["dependencies"]
                    .get_mut(dep_name)
                    .unwrap_or_else(|| {
                        panic!("{manifest_path} should contain {dep_name} dependency")
                    })
                    .as_table_mut()
                    .unwrap()
            } else {
                table["dependencies"]
                    .get_mut(dep_name)
                    .unwrap_or_else(|| {
                        panic!("{manifest_path} should contain {dep_name} dependency")
                    })
                    .as_table_mut()
                    .unwrap()
            };

            update_dep_path(dep, new_dep_path);
        }
        fn update_dev_dependency(
            table: &mut toml::map::Map<String, Value>,
            dep_name: &str,
            new_dep_path: &Utf8PathBuf,
        ) {
            let dep = if table.contains_key("workspace") {
                if table["workspace"].as_table().unwrap().contains_key("dev-dependencies")
                    && table["workspace"]["dev-dependencies"]
                        .as_table()
                        .unwrap()
                        .contains_key(dep_name)
                {
                    Some(table["workspace"]["dev-dependencies"][dep_name].as_table_mut().unwrap())
                } else {
                    None
                }
            } else if table.contains_key("dev-dependencies")
                && table["dev-dependencies"].as_table().unwrap().contains_key(dep_name)
            {
                Some(table["dev-dependencies"][dep_name].as_table_mut().unwrap())
            } else {
                None
            };

            if let Some(dep) = dep {
                update_dep_path(dep, new_dep_path);
            }
        }

        let mut contents = String::new();
        File::open(manifest_path)
            .and_then(|mut file| file.read_to_string(&mut contents))
            .unwrap_or_else(|_| panic!("Failed to read {manifest_path}"));

        let mut table = contents.parse::<Table>().expect("Failed to parse Scarb.toml");

        let root_path = dojo_core_path.parent().unwrap();

        update_dependency(manifest_path, &mut table, "dojo", dojo_core_path);
        update_dependency(
            manifest_path,
            &mut table,
            "dojo_cairo_macros",
            &root_path.join("macros"),
        );
        update_dev_dependency(&mut table, "dojo_snf_test", &root_path.join("dojo-snf-test"));
        update_dev_dependency(&mut table, "dojo_cairo_test", &root_path.join("dojo-cairo-test"));

        fs::write(manifest_path.to_path_buf(), table.to_string().as_bytes())
            .expect("Failed to write to Scarb.toml");
    }

    /// Copies a project to a new location, excluding the manifests and target directories.
    ///
    /// # Arguments
    ///
    /// * `source_dir` - The source directory to copy from.
    fn copy_project(
        source_dir: &Utf8PathBuf,
        destination_dir: &Utf8PathBuf,
        dojo_core_path: &Utf8PathBuf,
        ignore_dirs: &[&str],
    ) -> io::Result<()> {
        if !destination_dir.exists() {
            fs::create_dir_all(destination_dir)
                .unwrap_or_else(|_| panic!("Failed to create {destination_dir}"));
        }

        for entry in
            fs::read_dir(source_dir).unwrap_or_else(|_| panic!("Failed to read {:?}", source_dir))
        {
            let entry = entry.expect("Invalid entry");
            let path = entry.path();
            if path.is_dir() {
                let dir_name = match entry.file_name().into_string() {
                    Ok(name) => name,
                    Err(_) => continue, // Skip non UTF8 dirs.
                };

                if ignore_dirs.contains(&dir_name.as_str()) {
                    continue;
                }

                Self::copy_project(
                    &Utf8PathBuf::from_path_buf(path).unwrap(),
                    &destination_dir.join(dir_name),
                    dojo_core_path,
                    ignore_dirs,
                )
                .unwrap_or_else(|_| panic!("Failed to copy {:?}", entry.path()));
            } else {
                let file_name = entry.file_name().to_string_lossy().to_string();
                let dest_path = destination_dir.join(&file_name);

                fs::copy(&path, &dest_path).unwrap_or_else(|_| panic!("Failed to copy {:?}", path));

                if file_name == "Scarb.toml" {
                    Self::update_crates_in_manifest_file(&dest_path, dojo_core_path);
                }
            }
        }

        Ok(())
    }
}

// TODO RBA
// #[cfg(test)]
// mod tests {
// use std::fs::{self, File};
// use std::io::Write;
//
// use assert_fs::TempDir;
//
// use super::*;
//
// #[test]
// fn test_copy_project() {
// let temp_dir = TempDir::new().unwrap();
// let project_dir = temp_dir.path().join("project");
// let dest_dir = temp_dir.path().join("dest");
//
// fs::create_dir(&project_dir).unwrap();
// fs::create_dir(&dest_dir).unwrap();
//
// Create a file in the project directory
// let file_path = project_dir.join("file.txt");
// let mut file = File::create(file_path).unwrap();
// writeln!(file, "Hello, world!").unwrap();
//
// Create a subdirectory with a file in the project directory
// let sub_dir = project_dir.join("subdir");
// fs::create_dir(&sub_dir).unwrap();
// let sub_file_path = sub_dir.join("subfile.txt");
// let mut sub_file = File::create(sub_file_path).unwrap();
// writeln!(sub_file, "Hello, from subdir!").unwrap();
//
// Create a subdir that should be ignored
// let ignored_sub_dir = project_dir.join("manifests");
// fs::create_dir(&ignored_sub_dir).unwrap();
// let ignored_sub_file_path = ignored_sub_dir.join("ignored_file.txt");
// let mut ignored_sub_file = File::create(ignored_sub_file_path).unwrap();
// writeln!(ignored_sub_file, "This should be ignored!").unwrap();
//
// let ignore_dirs = ["manifests", "target"];
//
// copy_project_temp(
// &Utf8PathBuf::from(&project_dir.to_string_lossy()),
// &Utf8PathBuf::from(&dest_dir.to_string_lossy()),
// &Utf8PathBuf::from("../dojo/core"),
// &ignore_dirs,
// )
// .unwrap();
//
// Check that the file exists in the destination directory
// let dest_file_path = dest_dir.join("file.txt");
// assert!(dest_file_path.exists());
//
// Check that the subdirectory and its file exist in the destination directory
// let dest_sub_dir = dest_dir.join("subdir");
// let dest_sub_file_path = dest_sub_dir.join("subfile.txt");
// let dest_ignored_sub_dir = dest_sub_dir.join("manifests");
// assert!(dest_sub_dir.exists());
// assert!(dest_sub_file_path.exists());
// assert!(!dest_ignored_sub_dir.exists());
//
// Clean up
// temp_dir.close().unwrap();
// }
// }

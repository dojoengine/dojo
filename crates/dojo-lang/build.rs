use std::path::{Path, PathBuf};
use std::process::Command;

use cargo_metadata::MetadataCommand;

fn main() {
    commit_info();
    cairo_version();
}

fn commit_info() {
    if !Path::new("../../.git").exists() {
        return;
    }
    println!("cargo:rerun-if-changed=../../.git/index");
    let output = match Command::new("git")
        .arg("log")
        .arg("-1")
        .arg("--date=short")
        .arg("--format=%H %h %cd")
        .arg("--abbrev=9")
        .current_dir("..")
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return,
    };
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut parts = stdout.split_whitespace();
    let mut next = || parts.next().unwrap();
    println!("cargo:rustc-env=DOJO_COMMIT_HASH={}", next());
    println!("cargo:rustc-env=DOJO_COMMIT_SHORT_HASH={}", next());
    println!("cargo:rustc-env=DOJO_COMMIT_DATE={}", next())
}

fn cairo_version() {
    let cargo_lock = find_cargo_lock();
    println!("cargo:rerun-if-changed={}", cargo_lock.display());

    let metadata = MetadataCommand::new()
        .manifest_path("./Cargo.toml")
        .verbose(true)
        .exec()
        .expect("Failed to execute cargo metadata");

    let resolve = metadata.resolve.expect("Expected metadata resolve to be present.");

    let root = resolve.root.expect("Expected metadata resolve root to be present.");
    assert!(root.repr.starts_with("dojo"), "Expected metadata resolve root to be `dojo`.");

    let dojo_node = resolve.nodes.iter().find(|node| node.id == root).unwrap();
    let compiler_dep = dojo_node.deps.iter().find(|dep| dep.name == "cairo_lang_compiler").unwrap();
    let compiler_package = metadata.packages.iter().find(|pkg| pkg.id == compiler_dep.pkg).unwrap();

    let version = compiler_package.version.to_string();
    println!("cargo:rustc-env=DOJO_CAIRO_VERSION={version}");

    let mut rev = format!("refs/tags/v{version}");
    if let Some(source) = &compiler_package.source {
        let source = source.to_string();
        if source.starts_with("git+") {
            if let Some((_, commit)) = source.split_once('#') {
                println!("cargo:rustc-env=DOJO_CAIRO_COMMIT_HASH={commit}");
                let mut short_commit = commit.to_string();
                short_commit.truncate(9);
                println!("cargo:rustc-env=DOJO_CAIRO_SHORT_COMMIT_HASH={short_commit}");
                rev = commit.to_string();
            }
        }
    }
    println!("cargo:rustc-env=DOJO_CAIRO_COMMIT_REV={rev}");
}

fn find_cargo_lock() -> PathBuf {
    let in_workspace = PathBuf::from("../../Cargo.lock");
    if in_workspace.exists() {
        return in_workspace;
    }

    let in_package = PathBuf::from("Cargo.lock");
    if in_package.exists() {
        return in_package;
    }

    panic!(
        "Couldn't find Cargo.lock of this package. Something's wrong with build execution \
         environment."
    )
}

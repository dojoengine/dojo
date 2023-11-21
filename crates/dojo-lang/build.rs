use std::env;
use std::process::Command;

fn main() {
    let version = env!("CARGO_PKG_VERSION");
    let output = Command::new("git")
        .args(["rev-list", "-n", "1", &format!("v{version}")])
        .output()
        .expect("Failed to execute command");

    let git_hash = String::from_utf8(output.stdout).unwrap().trim().to_string();
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
}

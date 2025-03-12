use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=ui/");

    // Check if we're in a build script
    if std::env::var("CARGO_MANIFEST_DIR").is_ok() {
        // $CARGO_MANIFEST_DIR/ui/
        let ui_dir = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("ui");
        println!("Explorer UI directory: {}", ui_dir.display());

        // Update git submodule
        println!("UI directory is empty, updating git submodule...");
        let status = Command::new("git")
            .arg("submodule")
            .arg("update")
            .arg("--init")
            .arg("--recursive")
            .status()
            .expect("Failed to update git submodule");

        if !status.success() {
            panic!("Failed to update git submodule");
        }

        // Install dependencies if node_modules doesn't exist
        // $CARGO_MANIFEST_DIR/ui/node_modules
        if !Path::new(&ui_dir).join("node_modules").exists() {
            println!("Installing UI dependencies...");

            let status = Command::new("bun")
                .current_dir(&ui_dir)
                .arg("install")
                .status()
                .expect("Failed to install UI dependencies");

            if !status.success() {
                panic!("Failed to install UI dependencies");
            }
        }

        println!("Building UI...");
        let status = Command::new("bun")
            .current_dir(&ui_dir)
            .arg("run")
            .arg("build")
            .status()
            .expect("Failed to build UI");

        if !status.success() {
            panic!("Failed to build UI");
        }
    }
}

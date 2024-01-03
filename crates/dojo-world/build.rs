/// Script that generates the bindings for World and Executor contracts.
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use cairo_lang_starknet::contract_class::ContractClass;
use camino::Utf8PathBuf;
use scarb::core::{Config, TargetKind};
use scarb::ops::CompileOpts;

const SCARB_MANIFEST: &str = "../dojo-core/Scarb.toml";
const SCARB_MANIFEST_BACKUP: &str = "../dojo-core/bak.Scarb.toml";
const SCARB_LOCK: &str = "../dojo-core/Scarb.toml";
const SCARB_LOCK_BACKUP: &str = "../dojo-core/bak.Scarb.toml";
const WORLD_ARTIFACT: &str = "../dojo-core/target/dev/dojo_world.contract_class.json";
const EXECUTOR_ARTIFACT: &str = "../dojo-core/target/dev/dojo_executor.contract_class.json";
const OUT_DIR: &str = "./src/contracts/abi";

fn main() {
    // Only generate artifacts if not present for faster local compilation.
    if !Path::new(WORLD_ARTIFACT).exists() || !Path::new(EXECUTOR_ARTIFACT).exists() {
        compile_dojo_core();
    }

    let world_contract =
        serde_json::from_reader::<_, ContractClass>(File::open(WORLD_ARTIFACT).unwrap())
            .expect("Could not read World Contract Class file");

    write_binding_file(&format!("{OUT_DIR}/world.rs"), "WorldContract", world_contract);

    let executor_contract =
        serde_json::from_reader::<_, ContractClass>(File::open(EXECUTOR_ARTIFACT).unwrap())
            .expect("Could not read ExecutorContract Class file");

    write_binding_file(&format!("{OUT_DIR}/executor.rs"), "ExecutorContract", executor_contract);
}

fn rename_file(old_path: &str, new_path: &str) {
    let o = Path::new(old_path);
    let n = Path::new(new_path);
    fs::rename(o, n)
        .unwrap_or_else(|e| panic!("Could not rename file {old_path} into {new_path}: {e}"));
}

/// Writes a binding file using cainome inlined ABI for the given contract.
fn write_binding_file(file_name: &str, contract_name: &str, contract_class: ContractClass) {
    let mut file = File::create(file_name).expect("Could not create file");
    writeln!(
        file,
        "use cainome::rs::abigen;\n\nabigen!(\n    {},\n    r#\"{}\"#\n);",
        contract_name,
        serde_json::to_string(&contract_class.abi).unwrap()
    )
    .expect("Could not write Scarb.toml file");
}

/// Compiles dojo-core contracts targetting starknet contract without using dojo-plugin.
fn compile_dojo_core() {
    rename_file(SCARB_MANIFEST, SCARB_MANIFEST_BACKUP);
    rename_file(SCARB_LOCK, SCARB_LOCK_BACKUP);

    // Write new Scarb.toml file with starknet contract target.
    let mut file = File::create(SCARB_MANIFEST).expect("Could not create file");
    writeln!(
        file,
        r#"
[package]
cairo-version = "2.4.0"
name = "dojo"
version = "0.4.4"

[dependencies]
starknet = "2.4.0"

[[target.starknet-contract]]
sierra = true
"#,
    )
    .expect("Could not write Scarb.toml file");

    let path = Utf8PathBuf::from(SCARB_MANIFEST);
    let config = Config::builder(path.canonicalize_utf8().unwrap()).build().unwrap();
    let ws = scarb::ops::read_workspace(config.manifest_path(), &config)
        .expect("Could not read Scarb workspace");
    let packages = ws.members().map(|p| p.id).collect();

    scarb::ops::compile(
        packages,
        CompileOpts { include_targets: vec![], exclude_targets: vec![TargetKind::TEST] },
        &ws,
    )
    .expect("Could not run Scarb compile");

    rename_file(SCARB_MANIFEST_BACKUP, SCARB_MANIFEST);
    rename_file(SCARB_LOCK_BACKUP, SCARB_LOCK);
}

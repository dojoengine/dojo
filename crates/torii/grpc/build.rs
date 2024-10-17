use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir =
        PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR environment variable not set"));
    let target = std::env::var("TARGET").expect("TARGET environment variable not set");
    let feature_client = std::env::var("CARGO_FEATURE_CLIENT");
    let feature_server = std::env::var("CARGO_FEATURE_SERVER");

    if target.contains("wasm32") {
        if feature_server.is_ok() {
            panic!("feature `server` is not supported on target `{}`", target);
        }

        wasm_tonic_build::configure()
            .build_server(false)
            .build_client(feature_client.is_ok())
            .file_descriptor_set_path(out_dir.join("world_descriptor.bin"))
            .compile_protos(&["proto/world.proto"], &["proto"])?;
    } else {
        tonic_build::configure()
            .build_server(feature_server.is_ok())
            .build_client(feature_client.is_ok())
            .file_descriptor_set_path(out_dir.join("world_descriptor.bin"))
            .compile(&["proto/world.proto"], &["proto"])?;
    }

    println!("cargo:rerun-if-changed=proto");

    Ok(())
}

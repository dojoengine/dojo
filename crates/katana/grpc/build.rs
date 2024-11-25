use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir =
        PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR environment variable not set"));
    let feature_client = std::env::var("CARGO_FEATURE_CLIENT");
    let feature_server = std::env::var("CARGO_FEATURE_SERVER");

    tonic_build::configure()
	    // .build_server(feature_server.is_ok())
	    // .build_client(feature_client.is_ok())
        .build_transport(true)
        .file_descriptor_set_path(out_dir.join("starknet_descriptor.bin"))
        .compile(&["proto/starknet.proto"], &["proto"])?;

    println!("cargo:rerun-if-changed=proto");

    Ok(())
}

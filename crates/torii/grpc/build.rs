fn main() -> Result<(), Box<dyn std::error::Error>> {
    let target = std::env::var("TARGET").expect("failed to get TARGET environment variable");
    let feature_client = std::env::var("CARGO_FEATURE_CLIENT");
    let feature_server = std::env::var("CARGO_FEATURE_SERVER");

    if target.contains("wasm32") {
        if feature_server.is_ok() {
            panic!("feature `server` is not supported on target `{}`", target);
        }

        wasm_tonic_build::configure()
            .build_server(false)
            .build_client(feature_client.is_ok())
            .compile(&["proto/world.proto"], &["proto"])?;
    } else {
        tonic_build::configure()
            .build_server(feature_server.is_ok())
            .build_client(feature_client.is_ok())
            .compile(&["proto/world.proto"], &["proto"])?;
    }
    Ok(())
}

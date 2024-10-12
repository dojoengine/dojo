use std::collections::HashMap;
use std::fs;

use camino::Utf8PathBuf;
use dojo_test_utils::compiler;
use scarb::compiler::Profile;
use scarb::ops;
use url::Url;

use crate::contracts::naming::{get_filename_from_tag, TAG_SEPARATOR};
use crate::manifest::{CONTRACTS_DIR, MODELS_DIR, WORLD_CONTRACT_TAG};
use crate::metadata::{
    dojo_metadata_from_workspace, ArtifactMetadata, Uri, WorldMetadata, ABIS_DIR, BASE_DIR,
    MANIFESTS_DIR,
};

#[tokio::test]
async fn world_metadata_hash_and_upload() {
    let meta = WorldMetadata {
        name: "Test World".to_string(),
        seed: String::from("dojo_examples"),
        description: Some("A world used for testing".to_string()),
        cover_uri: Some(Uri::File("src/metadata_test_data/cover.png".into())),
        icon_uri: Some(Uri::File("src/metadata_test_data/cover.png".into())),
        website: Some(Url::parse("https://dojoengine.org").unwrap()),
        socials: Some(HashMap::from([("x".to_string(), "https://x.com/dojostarknet".to_string())])),
        artifacts: ArtifactMetadata {
            abi: Some(Uri::File("src/metadata_test_data/abi.json".into())),
            source: Some(Uri::File("src/metadata_test_data/source.cairo".into())),
        },
    };

    let _ = meta.upload().await.unwrap();
}

#[tokio::test]
async fn get_full_dojo_metadata_from_workspace() {
    let config =
        compiler::build_test_config("../../examples/spawn-and-move/Scarb.toml", Profile::DEV)
            .unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

    let profile = ws.config().profile();
    let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
    let manifest_dir = manifest_dir.join(MANIFESTS_DIR).join(profile.as_str());
    let target_dir = ws.target_dir().path_existent().unwrap();
    let target_dir = target_dir.join(profile.as_str());
    let abis_dir = manifest_dir.join(BASE_DIR).join(ABIS_DIR);

    let dojo_metadata =
        dojo_metadata_from_workspace(&ws).expect("No current package with dojo metadata found.");

    // env
    assert!(dojo_metadata.env.is_some());
    let env = dojo_metadata.env.unwrap();

    assert!(env.rpc_url.is_some());
    assert!(env.rpc_url.unwrap().eq("http://localhost:5050/"));

    assert!(env.account_address.is_some());
    assert!(env
        .account_address
        .unwrap()
        .eq("0x5a37d83d451063858217e9c510d6f45d6bd37ff8664a7c0466329316f7a2891"));

    assert!(env.private_key.is_some());
    assert!(env
        .private_key
        .unwrap()
        .eq("0x1800000000300000180000000000030000000000003006001800006600"));

    assert!(env.world_address.is_some());

    assert!(env.keystore_path.is_none());
    assert!(env.keystore_password.is_none());

    // world
    assert_eq!(dojo_metadata.world.name, "example");

    assert!(dojo_metadata.world.description.is_some());
    assert!(dojo_metadata.world.description.unwrap().eq("example world"));

    assert!(dojo_metadata.world.cover_uri.is_none());
    assert!(dojo_metadata.world.icon_uri.is_none());
    assert!(dojo_metadata.world.website.is_none());
    assert!(dojo_metadata.world.socials.is_none());

    let world_filename = get_filename_from_tag(WORLD_CONTRACT_TAG);
    assert!(dojo_metadata.world.artifacts.abi.is_some(), "No abi for {world_filename}");
    let abi = dojo_metadata.world.artifacts.abi.unwrap();
    assert_eq!(
        abi,
        Uri::File(abis_dir.join(format!("{world_filename}.json")).into()),
        "Bad abi for {world_filename}",
    );

    let artifacts = get_artifacts_from_manifest(&manifest_dir);

    for (subdir, filename) in artifacts {
        let tag = get_tag_from_filename(&filename);
        let resource = dojo_metadata.resources_artifacts.get(&tag);

        assert!(resource.is_some(), "bad resource metadata for {}", tag);
        let resource = resource.unwrap();

        check_artifact(
            resource.artifacts.clone(),
            filename,
            &abis_dir.join(subdir),
            &target_dir.join(subdir),
        );
    }
}

fn check_artifact(
    artifact: ArtifactMetadata,
    basename: String,
    abis_dir: &Utf8PathBuf,
    source_dir: &Utf8PathBuf,
) {
    assert!(artifact.abi.is_some(), "No abi for {}", basename);
    let abi = artifact.abi.unwrap();
    assert_eq!(
        abi,
        Uri::File(abis_dir.join(format!("{basename}.json")).into()),
        "Bad abi for {}",
        basename
    );

    assert!(artifact.source.is_some(), "No source for {}", basename);
    let source = artifact.source.unwrap();
    assert_eq!(
        source,
        Uri::File(source_dir.join(format!("{basename}.cairo")).into()),
        "Bad source for {}",
        basename
    );
}

fn get_artifacts_from_manifest(manifest_dir: &Utf8PathBuf) -> Vec<(&str, String)> {
    let contracts_dir = manifest_dir.join(BASE_DIR).join(CONTRACTS_DIR);
    let models_dir = manifest_dir.join(BASE_DIR).join(MODELS_DIR);

    let mut artifacts = vec![];

    // models
    for entry in fs::read_dir(models_dir).unwrap().flatten() {
        let filename = entry.path().file_stem().unwrap().to_string_lossy().to_string();
        artifacts.push((MODELS_DIR, filename));
    }

    // contracts
    for entry in fs::read_dir(contracts_dir).unwrap().flatten() {
        let filename = entry.path().file_stem().unwrap().to_string_lossy().to_string();
        artifacts.push((CONTRACTS_DIR, filename));
    }

    artifacts
}

fn get_tag_from_filename(filename: &str) -> String {
    let parts = filename.split(TAG_SEPARATOR).collect::<Vec<_>>();
    assert!(parts.len() >= 2);
    format!("{}{TAG_SEPARATOR}{}", parts[0], parts[1])
}

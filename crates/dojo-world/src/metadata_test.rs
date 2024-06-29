use std::collections::HashMap;
use std::fs;

use camino::Utf8PathBuf;
use dojo_test_utils::compiler;
use scarb::ops;
use url::Url;

use crate::metadata::{
    dojo_metadata_from_workspace, ArtifactMetadata, ProjectMetadata, Uri, WorldMetadata, ABIS_DIR,
    BASE_DIR, MANIFESTS_DIR, SOURCES_DIR,
};

#[test]
fn check_metadata_deserialization() {
    let metadata: ProjectMetadata = toml::from_str(
        r#"
[env]
rpc_url = "http://localhost:5050/"
account_address = "0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973"
private_key = "0x1800000000300000180000000000030000000000003006001800006600"
keystore_path = "test/"
keystore_password = "dojo"
world_address = "0x0248cacaeac64c45be0c19ee8727e0bb86623ca7fa3f0d431a6c55e200697e5a"

[world]
name = "example"
description = "example world"
cover_uri = "file://example_cover.png"
icon_uri = "file://example_icon.png"
website = "https://dojoengine.org"
socials.x = "https://x.com/dojostarknet"
        "#,
    )
    .unwrap();

    assert!(metadata.env.is_some());
    let env = metadata.env.unwrap();

    assert_eq!(env.rpc_url(), Some("http://localhost:5050/"));
    assert_eq!(
        env.account_address(),
        Some("0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973")
    );
    assert_eq!(
        env.private_key(),
        Some("0x1800000000300000180000000000030000000000003006001800006600")
    );
    assert_eq!(env.keystore_path(), Some("test/"));
    assert_eq!(env.keystore_password(), Some("dojo"));
    assert_eq!(
        env.world_address(),
        Some("0x0248cacaeac64c45be0c19ee8727e0bb86623ca7fa3f0d431a6c55e200697e5a")
    );

    assert!(metadata.world.is_some());
    let world = metadata.world.unwrap();

    assert_eq!(world.name(), Some("example"));
    assert_eq!(world.description(), Some("example world"));
    assert_eq!(world.cover_uri, Some(Uri::File("example_cover.png".into())));
    assert_eq!(world.icon_uri, Some(Uri::File("example_icon.png".into())));
    assert_eq!(world.website, Some(Url::parse("https://dojoengine.org").unwrap()));
    assert_eq!(world.socials.unwrap().get("x"), Some(&"https://x.com/dojostarknet".to_string()));
}

#[tokio::test]
async fn world_metadata_hash_and_upload() {
    let meta = WorldMetadata {
        name: Some("Test World".to_string()),
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
async fn parse_world_metadata_without_socials() {
    let metadata: ProjectMetadata = toml::from_str(
        r#"
[env]
rpc_url = "http://localhost:5050/"
account_address = "0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973"
private_key = "0x1800000000300000180000000000030000000000003006001800006600"
keystore_path = "test/"
keystore_password = "dojo"
world_address = "0x0248cacaeac64c45be0c19ee8727e0bb86623ca7fa3f0d431a6c55e200697e5a"

[world]
name = "example"
description = "example world"
cover_uri = "file://example_cover.png"
icon_uri = "file://example_icon.png"
website = "https://dojoengine.org"
# socials.x = "https://x.com/dojostarknet"
        "#,
    )
    .unwrap();

    assert!(metadata.world.is_some());
}

#[tokio::test]
async fn get_full_dojo_metadata_from_workspace() {
    let config = compiler::build_test_config("../../examples/spawn-and-move/Scarb.toml").unwrap();
    let ws = ops::read_workspace(config.manifest_path(), &config)
        .unwrap_or_else(|op| panic!("Error building workspace: {op:?}"));

    let profile = ws.config().profile();
    let manifest_dir = ws.manifest_path().parent().unwrap().to_path_buf();
    let manifest_dir = manifest_dir.join(MANIFESTS_DIR).join(profile.as_str());
    let target_dir = ws.target_dir().path_existent().unwrap();
    let sources_dir = target_dir.join(profile.as_str()).join(SOURCES_DIR);
    let abis_dir = manifest_dir.join(ABIS_DIR).join(BASE_DIR);

    let dojo_metadata =
        dojo_metadata_from_workspace(&ws).expect("No current package with dojo metadata found.");

    // env
    assert!(dojo_metadata.env.is_some());
    let env = dojo_metadata.env.unwrap();

    assert!(env.rpc_url.is_some());
    assert!(env.rpc_url.unwrap().eq("http://localhost:5050/"));

    assert!(env.account_address.is_some());
    assert!(
        env.account_address
            .unwrap()
            .eq("0x6162896d1d7ab204c7ccac6dd5f8e9e7c25ecd5ae4fcb4ad32e57786bb46e03")
    );

    assert!(env.private_key.is_some());
    assert!(
        env.private_key.unwrap().eq("0x1800000000300000180000000000030000000000003006001800006600")
    );

    assert!(env.world_address.is_some());
    assert_eq!(
        env.world_address.unwrap(),
        "0x07efebb0c2d4cc285d48a97a7174def3be7fdd6b7bd29cca758fa2e17e03ef30"
    );

    assert!(env.keystore_path.is_none());
    assert!(env.keystore_password.is_none());

    // world
    assert!(dojo_metadata.world.name.is_some());
    assert!(dojo_metadata.world.name.unwrap().eq("example"));

    assert!(dojo_metadata.world.description.is_some());
    assert!(dojo_metadata.world.description.unwrap().eq("example world"));

    assert!(dojo_metadata.world.cover_uri.is_none());
    assert!(dojo_metadata.world.icon_uri.is_none());
    assert!(dojo_metadata.world.website.is_none());
    assert!(dojo_metadata.world.socials.is_none());

    check_artifact(
        dojo_metadata.world.artifacts,
        "dojo_world_world".to_string(),
        &abis_dir,
        &sources_dir,
    );

    let artifacts = get_artifacts_from_manifest(&manifest_dir);

    dbg!(&artifacts);
    for (abi_subdir, name) in artifacts {
        let resource = dojo_metadata.resources_artifacts.get(&name);
        dbg!(&dojo_metadata.resources_artifacts);
        assert!(resource.is_some(), "bad resource metadata for {}", name);
        let resource = resource.unwrap();

        let sanitized_name = name.replace("::", "_");

        check_artifact(
            resource.artifacts.clone(),
            sanitized_name,
            &abis_dir.join(abi_subdir),
            &sources_dir,
        );
    }
}

fn check_artifact(
    artifact: ArtifactMetadata,
    name: String,
    abis_dir: &Utf8PathBuf,
    sources_dir: &Utf8PathBuf,
) {
    assert!(artifact.abi.is_some());
    let abi = artifact.abi.unwrap();
    assert_eq!(
        abi,
        Uri::File(abis_dir.join(format!("{name}.json")).into()),
        "Bad abi for {}",
        name
    );

    assert!(artifact.source.is_some());
    let source = artifact.source.unwrap();
    assert_eq!(
        source,
        Uri::File(sources_dir.join(format!("{name}.cairo")).into()),
        "Bad source for {}",
        name
    );
}

fn get_artifacts_from_manifest(manifest_dir: &Utf8PathBuf) -> Vec<(String, String)> {
    let contracts_dir = manifest_dir.join(BASE_DIR).join("contracts");
    let models_dir = manifest_dir.join(BASE_DIR).join("models");

    let mut artifacts = vec![];

    // models
    for entry in fs::read_dir(models_dir).unwrap().flatten() {
        let name = entry.path().file_stem().unwrap().to_string_lossy().to_string();
        let name = name.replace("_models_", "::models::");
        // Some models are inside actions, we need a better way to gather those.
        let name = name.replace("_actions_", "::actions::");
        let name = name.replace("::actions_", "::actions::");

        let name = name.replace("_others_", "::others::");
        let name = name.replace("::others_", "::others::");

        let name = name.replace("_mock_token_", "::mock_token::");
        let name = name.replace("::mock_token_", "::mock_token::");

        let name = name.replace("_something_", "::something::");
        let name = name.replace("::something_", "::something::");
        artifacts.push(("models".to_string(), name));
    }

    // contracts
    for entry in fs::read_dir(contracts_dir).unwrap().flatten() {
        let name = entry.path().file_stem().unwrap().to_string_lossy().to_string();
        let name = name.replace("_actions_", "::actions::");
        let name = name.replace("_others_", "::others::");
        let name = name.replace("_mock_token_", "::mock_token::");
        let name = name.replace("_something_", "::something::");
        artifacts.push(("contracts".to_string(), name));
    }

    artifacts
}

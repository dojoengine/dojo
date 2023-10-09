use super::WorldMetadata;
use crate::metadata::{Metadata, Uri};

#[test]
fn check_metadata_deserialization() {
    let metadata: Metadata = toml::from_str(
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
}

#[tokio::test]
async fn world_metadata_hash_and_upload() {
    let meta = WorldMetadata {
        name: Some("Test World".to_string()),
        description: Some("A world used for testing".to_string()),
        cover_uri: Some(Uri::File("src/metadata_test_data/cover.png".into())),
        icon_uri: None,
    };

    let _ = meta.upload().await.unwrap();
}

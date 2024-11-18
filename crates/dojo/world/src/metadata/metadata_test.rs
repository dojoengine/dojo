use core::str;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use starknet_crypto::Felt;
use url::Url;

use super::fake_metadata_service::FakeMetadataService;
use super::metadata_service::MetadataService;
use super::metadata_storage::MetadataStorage;
use crate::config::metadata_config::{ResourceMetadata, WorldMetadata};
use crate::uri::Uri;

/// Helper function to create a local file absolute path
/// from a relative path.
fn test_file_path(filename: &str) -> PathBuf {
    fs::canonicalize(
        PathBuf::from_str(&format!("./src/metadata/metadata_test_data/{}", filename)).unwrap(),
    )
    .unwrap()
}

/// Helper function to build a WorldMetadata for tests.
fn build_world_metadata() -> WorldMetadata {
    WorldMetadata {
        name: "world".to_string(),
        seed: "world seed".to_string(),
        description: Some("world description".to_string()),
        cover_uri: Some(Uri::File(test_file_path("cover.png"))),
        icon_uri: Some(Uri::File(test_file_path("icon.png"))),
        website: Some(Url::parse("https://my_world.com").expect("parsing failed")),
        socials: Some(HashMap::from([
            ("twitter".to_string(), "twitter_url".to_string()),
            ("discord".to_string(), "discord_url".to_string()),
        ])),
    }
}

/// Helper function to build a ResourceMetadata for tests.
fn build_resource_metadata() -> ResourceMetadata {
    ResourceMetadata {
        name: "my model".to_string(),
        description: Some("my model description".to_string()),
        icon_uri: Some(Uri::File(test_file_path("icon.png"))),
    }
}

// Helper function to check IPFS URI.
fn assert_ipfs_uri(uri: &Option<Uri>) {
    if let Some(uri) = uri {
        assert!(uri.to_string().starts_with("ipfs://"));
    }
}

// Helper function to check IPFS content.
async fn assert_ipfs_content(service: &FakeMetadataService, uri: String, path: PathBuf) {
    let ipfs_data = service.get(uri).await.expect("read metadata failed");
    let expected_data = std::fs::read(path).expect("read local data failed");
    assert_eq!(ipfs_data, expected_data);
}

#[tokio::test]
async fn test_world_metadata() {
    let mut metadata_service = FakeMetadataService::default();

    let world_metadata = build_world_metadata();

    // first metadata upload without existing hash.
    let res = world_metadata.upload_if_changed(&mut metadata_service, Felt::ZERO).await;

    let (current_uri, current_hash) = if let Ok(Some(res)) = res {
        res
    } else {
        panic!("Upload failed");
    };

    // no change => the upload is not done.
    let res = world_metadata.upload_if_changed(&mut metadata_service, current_hash).await;

    assert!(res.is_ok());
    assert!(res.unwrap().is_none());

    // different hash => metadata are reuploaded.
    let res =
        world_metadata.upload_if_changed(&mut metadata_service, current_hash + Felt::ONE).await;

    let (new_uri, new_hash) = if let Ok(Some(res)) = res {
        res
    } else {
        panic!("Upload failed");
    };

    assert_eq!(new_uri, current_uri);
    assert_eq!(new_hash, current_hash);

    // read back the metadata from service to be sure it is correctly written
    let read_metadata = metadata_service.get(current_uri).await.expect("read metadata failed");

    let read_metadata = str::from_utf8(&read_metadata);
    assert!(read_metadata.is_ok());

    let read_metadata = serde_json::from_str::<WorldMetadata>(read_metadata.unwrap());
    assert!(read_metadata.is_ok());

    let read_metadata = read_metadata.unwrap();

    assert_eq!(read_metadata.name, "world".to_string());
    assert_eq!(read_metadata.seed, "world seed".to_string());
    assert_eq!(read_metadata.description, Some("world description".to_string()));
    assert_eq!(read_metadata.website, Some(Url::parse("https://my_world.com").unwrap()));
    assert_eq!(
        read_metadata.socials,
        Some(HashMap::from([
            ("twitter".to_string(), "twitter_url".to_string()),
            ("discord".to_string(), "discord_url".to_string()),
        ]))
    );

    assert_ipfs_uri(&read_metadata.cover_uri);
    assert_ipfs_content(
        &metadata_service,
        read_metadata.cover_uri.unwrap().to_string(),
        fs::canonicalize(PathBuf::from_str("./src/metadata/metadata_test_data/cover.png").unwrap())
            .unwrap(),
    )
    .await;

    assert_ipfs_uri(&read_metadata.icon_uri);
    assert_ipfs_content(
        &metadata_service,
        read_metadata.icon_uri.unwrap().to_string(),
        fs::canonicalize(PathBuf::from_str("./src/metadata/metadata_test_data/icon.png").unwrap())
            .unwrap(),
    )
    .await;
}

#[tokio::test]
async fn test_resource_metadata() {
    let mut metadata_service = FakeMetadataService::default();

    let resource_metadata = build_resource_metadata();

    // first metadata upload without existing hash.
    let res = resource_metadata.upload_if_changed(&mut metadata_service, Felt::ZERO).await;
    assert!(res.is_ok());
    let res = res.unwrap();

    assert!(res.is_some());
    let (current_uri, current_hash) = res.unwrap();

    // no change => the upload is not done.
    let res = resource_metadata.upload_if_changed(&mut metadata_service, current_hash).await;
    assert!(res.is_ok());
    let res = res.unwrap();

    assert!(res.is_none());

    // different hash => metadata are reuploaded.
    let res =
        resource_metadata.upload_if_changed(&mut metadata_service, current_hash + Felt::ONE).await;
    assert!(res.is_ok());
    let res = res.unwrap();

    assert!(res.is_some());
    let (new_uri, new_hash) = res.unwrap();

    assert_eq!(new_uri, current_uri);
    assert_eq!(new_hash, current_hash);

    // read back the metadata stored on IPFS to be sure it is correctly written
    let read_metadata = metadata_service.get(current_uri).await.expect("read metadata failed");

    let read_metadata = str::from_utf8(&read_metadata);
    assert!(read_metadata.is_ok());

    let read_metadata = serde_json::from_str::<ResourceMetadata>(read_metadata.unwrap());
    assert!(read_metadata.is_ok());

    let read_metadata = read_metadata.unwrap();

    assert_eq!(read_metadata.name, "my model".to_string());
    assert_eq!(read_metadata.description, Some("my model description".to_string()));

    assert_ipfs_uri(&read_metadata.icon_uri);
    assert_ipfs_content(
        &metadata_service,
        read_metadata.icon_uri.unwrap().to_string(),
        fs::canonicalize(PathBuf::from_str("./src/metadata/metadata_test_data/icon.png").unwrap())
            .unwrap(),
    )
    .await;
}

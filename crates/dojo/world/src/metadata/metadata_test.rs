use core::str;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use starknet_crypto::Felt;
use url::Url;

use crate::metadata::ipfs::IpfsClient;
use crate::metadata::{MetadataStorage, ResourceMetadata, WorldMetadata};
use crate::uri::Uri;

fn build_world_metadata() -> WorldMetadata {
    WorldMetadata {
        name: "world".to_string(),
        seed: "world seed".to_string(),
        description: Some("world description".to_string()),
        cover_uri: Some(Uri::File(
            fs::canonicalize(
                PathBuf::from_str("./src/metadata/metadata_test_data/cover.png").unwrap(),
            )
            .unwrap(),
        )),
        icon_uri: Some(Uri::File(
            fs::canonicalize(
                PathBuf::from_str("./src/metadata/metadata_test_data/icon.png").unwrap(),
            )
            .unwrap(),
        )),
        website: Some(Url::parse("https://my_world.com").expect("parsing failed")),
        socials: Some(HashMap::from([
            ("twitter".to_string(), "twitter_url".to_string()),
            ("discord".to_string(), "discord_url".to_string()),
        ])),
    }
}

fn build_resource_metadata() -> ResourceMetadata {
    ResourceMetadata {
        name: "my model".to_string(),
        description: Some("my model description".to_string()),
        icon_uri: Some(Uri::File(
            fs::canonicalize(
                PathBuf::from_str("./src/metadata/metadata_test_data/icon.png").unwrap(),
            )
            .unwrap(),
        )),
    }
}

fn assert_ipfs_uri(uri: &Option<Uri>) {
    if let Some(uri) = uri {
        assert!(uri.to_string().starts_with("ipfs://"));
    }
}

async fn assert_ipfs_content(uri: String, path: PathBuf) {
    let ipfs_client = IpfsClient::new().expect("Ipfs client failed");
    let ipfs_data = ipfs_client.get(uri).await.expect("read metadata failed");
    let expected_data = std::fs::read(path).expect("read local data failed");

    assert_eq!(ipfs_data, expected_data);
}

#[tokio::test]
async fn test_world_metadata() {
    let world_metadata = build_world_metadata();

    // first metadata upload without existing hash.
    let res = world_metadata.upload_if_changed(Felt::ZERO).await;

    let (current_uri, current_hash) = if let Ok(Some(res)) = res {
        res
    } else {
        panic!("Upload failed");
    };

    // no change => the upload is not done.
    let res = world_metadata.upload_if_changed(current_hash).await;

    assert!(res.is_ok());
    assert!(res.unwrap().is_none());

    // different hash => metadata are reuploaded.
    let res = world_metadata.upload_if_changed(current_hash + Felt::ONE).await;

    let (new_uri, new_hash) = if let Ok(Some(res)) = res {
        res
    } else {
        panic!("Upload failed");
    };

    assert_eq!(new_uri, current_uri);
    assert_eq!(new_hash, current_hash);

    // read back the metadata stored on IPFS to be sure it is correctly written
    let ipfs_client = IpfsClient::new().expect("Ipfs client failed");
    let read_metadata = ipfs_client.get(current_uri).await.expect("read metadata failed");

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
        read_metadata.cover_uri.unwrap().to_string(),
        fs::canonicalize(PathBuf::from_str("./src/metadata/metadata_test_data/cover.png").unwrap())
            .unwrap(),
    )
    .await;

    assert_ipfs_uri(&read_metadata.icon_uri);
    assert_ipfs_content(
        read_metadata.icon_uri.unwrap().to_string(),
        fs::canonicalize(PathBuf::from_str("./src/metadata/metadata_test_data/icon.png").unwrap())
            .unwrap(),
    )
    .await;

    // TODO: would be nice to fake IpfsClient for tests
}

#[tokio::test]
async fn test_resource_metadata() {
    let resource_metadata = build_resource_metadata();

    // first metadata upload without existing hash.
    let res = resource_metadata.upload_if_changed(Felt::ZERO).await;
    assert!(res.is_ok());
    let res = res.unwrap();

    assert!(res.is_some());
    let (current_uri, current_hash) = res.unwrap();

    // no change => the upload is not done.
    let res = resource_metadata.upload_if_changed(current_hash).await;
    assert!(res.is_ok());
    let res = res.unwrap();

    assert!(res.is_none());

    // different hash => metadata are reuploaded.
    let res = resource_metadata.upload_if_changed(current_hash + Felt::ONE).await;
    assert!(res.is_ok());
    let res = res.unwrap();

    assert!(res.is_some());
    let (new_uri, new_hash) = res.unwrap();

    assert_eq!(new_uri, current_uri);
    assert_eq!(new_hash, current_hash);

    // read back the metadata stored on IPFS to be sure it is correctly written
    let ipfs_client = IpfsClient::new().expect("Ipfs client failed");
    let read_metadata = ipfs_client.get(current_uri).await.expect("read metadata failed");

    let read_metadata = str::from_utf8(&read_metadata);
    assert!(read_metadata.is_ok());

    let read_metadata = serde_json::from_str::<ResourceMetadata>(read_metadata.unwrap());
    assert!(read_metadata.is_ok());

    let read_metadata = read_metadata.unwrap();

    assert_eq!(read_metadata.name, "my model".to_string());
    assert_eq!(read_metadata.description, Some("my model description".to_string()));

    assert_ipfs_uri(&read_metadata.icon_uri);
    assert_ipfs_content(
        read_metadata.icon_uri.unwrap().to_string(),
        fs::canonicalize(PathBuf::from_str("./src/metadata/metadata_test_data/icon.png").unwrap())
            .unwrap(),
    )
    .await;
}

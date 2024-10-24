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
        cover_uri: Some(Uri::File("src/metadata/metadata_test_data/cover.png".into())),
        icon_uri: Some(Uri::File("src/metadata/metadata_test_data/cover.png".into())),
        website: Some(Url::parse("https://dojoengine.org").unwrap()),
        socials: Some(HashMap::from([("x".to_string(), "https://x.com/dojostarknet".to_string())])),
        artifacts: ArtifactMetadata {
            abi: Some(Uri::File("src/metadata_test_data/abi.json".into())),
            source: Some(Uri::File("src/metadata_test_data/source.cairo".into())),
        },
    };

    let _ = meta.upload().await.unwrap();
}

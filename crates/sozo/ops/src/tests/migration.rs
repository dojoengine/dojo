use std::str;

use camino::Utf8Path;
use dojo_lang::compiler::{BASE_DIR, MANIFESTS_DIR};
use dojo_test_utils::compiler::build_full_test_config;
use dojo_test_utils::sequencer::{
    get_default_test_starknet_config, SequencerConfig, StarknetConfig, TestSequencer,
};
use dojo_world::contracts::WorldContractReader;
use dojo_world::manifest::{BaseManifest, DeploymentManifest, WORLD_CONTRACT_NAME};
use dojo_world::metadata::{
    dojo_metadata_from_workspace, ArtifactMetadata, DojoMetadata, Uri, WorldMetadata,
    IPFS_CLIENT_URL, IPFS_PASSWORD, IPFS_USERNAME,
};
use dojo_world::migration::strategy::prepare_for_migration;
use dojo_world::migration::world::WorldDiff;
use dojo_world::migration::TxnConfig;
use futures::TryStreamExt;
use ipfs_api_backend_hyper::{HyperBackend, IpfsApi, IpfsClient, TryFromUri};
use starknet::accounts::{ExecutionEncoding, SingleOwnerAccount};
use starknet::core::chain_id;
use starknet::core::types::{BlockId, BlockTag};
use starknet::core::utils::{get_selector_from_name, parse_cairo_short_string};
use starknet::macros::felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
use starknet::signers::{LocalWallet, SigningKey};
use starknet_crypto::FieldElement;

use super::setup::{load_config, setup_migration, setup_ws};
use crate::migration::{execute_strategy, upload_metadata};
use crate::utils::get_contract_address_from_reader;

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_auto_mine() {
    let config = load_config();
    let ws = setup_ws(&config);

    let mut migration = setup_migration().unwrap();

    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    execute_strategy(&ws, &mut migration, &account, TxnConfig::default()).await.unwrap();

    sequencer.stop().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_block_time() {
    let config = load_config();
    let ws = setup_ws(&config);

    let mut migration = setup_migration().unwrap();

    let sequencer = TestSequencer::start(
        SequencerConfig { block_time: Some(1000), ..Default::default() },
        get_default_test_starknet_config(),
    )
    .await;

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    execute_strategy(&ws, &mut migration, &account, TxnConfig::default()).await.unwrap();
    sequencer.stop().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_small_fee_multiplier_will_fail() {
    let config = load_config();
    let ws = setup_ws(&config);

    let mut migration = setup_migration().unwrap();

    let sequencer = TestSequencer::start(
        Default::default(),
        StarknetConfig { disable_fee: false, ..Default::default() },
    )
    .await;

    let account = SingleOwnerAccount::new(
        JsonRpcClient::new(HttpTransport::new(sequencer.url())),
        LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            sequencer.raw_account().private_key,
        )),
        sequencer.raw_account().account_address,
        chain_id::TESTNET,
        ExecutionEncoding::New,
    );

    assert!(
        execute_strategy(
            &ws,
            &mut migration,
            &account,
            TxnConfig { fee_estimate_multiplier: Some(0.2f64), ..Default::default() },
        )
        .await
        .is_err()
    );
    sequencer.stop().unwrap();
}

#[test]
fn migrate_world_without_seed_will_fail() {
    let profile_name = "dev";
    let base = "../../../examples/spawn-and-move";
    let target_dir = format!("{}/target/dev", base);
    let manifest = BaseManifest::load_from_path(
        &Utf8Path::new(base).to_path_buf().join(MANIFESTS_DIR).join(profile_name).join(BASE_DIR),
    )
    .unwrap();
    let world = WorldDiff::compute(manifest, None);
    let res = prepare_for_migration(None, None, &Utf8Path::new(&target_dir).to_path_buf(), world);
    assert!(res.is_err_and(|e| e.to_string().contains("Missing seed for World deployment.")))
}

#[tokio::test]
async fn migration_from_remote() {
    let config = load_config();
    let ws = setup_ws(&config);

    let base = "../../../examples/spawn-and-move";
    let target_dir = format!("{}/target/dev", base);

    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let account = SingleOwnerAccount::new(
        JsonRpcClient::new(HttpTransport::new(sequencer.url())),
        LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            sequencer.raw_account().private_key,
        )),
        sequencer.raw_account().account_address,
        chain_id::TESTNET,
        ExecutionEncoding::New,
    );

    let profile_name = ws.current_profile().unwrap().to_string();

    let manifest = BaseManifest::load_from_path(
        &Utf8Path::new(base).to_path_buf().join(MANIFESTS_DIR).join(&profile_name).join(BASE_DIR),
    )
    .unwrap();

    let world = WorldDiff::compute(manifest, None);

    let mut migration = prepare_for_migration(
        None,
        Some(felt!("0x12345")),
        &Utf8Path::new(&target_dir).to_path_buf(),
        world,
    )
    .unwrap();

    execute_strategy(&ws, &mut migration, &account, TxnConfig::default()).await.unwrap();

    let local_manifest = BaseManifest::load_from_path(
        &Utf8Path::new(base).to_path_buf().join(MANIFESTS_DIR).join(&profile_name).join(BASE_DIR),
    )
    .unwrap();

    let remote_manifest = DeploymentManifest::load_from_remote(
        JsonRpcClient::new(HttpTransport::new(sequencer.url())),
        migration.world_address().unwrap(),
    )
    .await
    .unwrap();

    sequencer.stop().unwrap();

    assert_eq!(local_manifest.world.inner.class_hash, remote_manifest.world.inner.class_hash);
    assert_eq!(local_manifest.models.len(), remote_manifest.models.len());
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_metadata() {
    let config = build_full_test_config("../../../examples/spawn-and-move/Scarb.toml", false)
        .unwrap_or_else(|c| panic!("Error loading config: {c:?}"));
    let ws = setup_ws(&config);

    let mut migration = setup_migration().unwrap();

    let sequencer =
        TestSequencer::start(SequencerConfig::default(), get_default_test_starknet_config()).await;

    let mut account = sequencer.account();
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let output =
        execute_strategy(&ws, &mut migration, &account, TxnConfig::default()).await.unwrap();

    let res = upload_metadata(&ws, &account, output.clone(), TxnConfig::default()).await;
    assert!(res.is_ok());

    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(output.world_address, &provider);

    let client = IpfsClient::from_str(IPFS_CLIENT_URL)
        .unwrap_or_else(|_| panic!("Unable to initialize the IPFS Client"))
        .with_credentials(IPFS_USERNAME, IPFS_PASSWORD);

    let dojo_metadata = dojo_metadata_from_workspace(&ws);

    // check world metadata
    let resource = world_reader.metadata(&FieldElement::ZERO).call().await.unwrap();
    let element_name = WORLD_CONTRACT_NAME.to_string();

    let full_uri = get_and_check_metadata_uri(&element_name, &resource.metadata_uri);
    let resource_bytes = get_ipfs_resource_data(&client, &element_name, &full_uri).await;

    let metadata = resource_bytes_to_world_metadata(&resource_bytes, &element_name);

    assert_eq!(metadata.name, dojo_metadata.world.name, "");
    assert_eq!(metadata.description, dojo_metadata.world.description, "");
    assert_eq!(metadata.cover_uri, dojo_metadata.world.cover_uri, "");
    assert_eq!(metadata.icon_uri, dojo_metadata.world.icon_uri, "");
    assert_eq!(metadata.website, dojo_metadata.world.website, "");
    assert_eq!(metadata.socials, dojo_metadata.world.socials, "");

    check_artifact_fields(
        &client,
        &metadata.artifacts,
        &dojo_metadata.world.artifacts,
        &element_name,
    )
    .await;

    // check model metadata
    for m in migration.models {
        let selector = get_selector_from_name(&m.diff.name).unwrap();
        check_artifact_metadata(&client, &world_reader, selector, &m.diff.name, &dojo_metadata)
            .await;
    }

    // check contract metadata
    for c in migration.contracts {
        let contract_address =
            get_contract_address_from_reader(&world_reader, c.diff.name.clone()).await.unwrap();
        check_artifact_metadata(
            &client,
            &world_reader,
            contract_address,
            &c.diff.name,
            &dojo_metadata,
        )
        .await;
    }
}

/// Get the hash from a IPFS URI
///
/// # Arguments
///
/// * `uri` - a full IPFS URI
///
/// # Returns
///
/// A [`String`] containing the hash from the URI.
fn get_hash_from_uri(uri: &str) -> String {
    let hash = match uri.strip_prefix("ipfs://") {
        Some(s) => s.to_string(),
        None => uri.to_owned(),
    };
    match hash.strip_suffix('/') {
        Some(s) => s.to_string(),
        None => hash,
    }
}

/// Check a metadata field which refers to a file.
///
/// # Arguments
///
/// * `client` - a IPFS client.
/// * `uri` - the IPFS URI of the abi field.
/// * `expected_uri` - the URI of the expected file.
/// * `field_name` - the field name.
/// * `element_name` - the fully qualified name of the element linked to this field.
async fn check_file_field(
    client: &HyperBackend,
    uri: &Uri,
    expected_uri: &Uri,
    field_name: String,
    element_name: &String,
) {
    if let Uri::Ipfs(uri) = uri {
        let resource_data = get_ipfs_resource_data(client, element_name, uri).await;
        assert!(
            !resource_data.is_empty(),
            "{field_name} IPFS artifact for {} is empty",
            element_name
        );

        if let Uri::File(f) = expected_uri {
            let file_content = std::fs::read_to_string(f).unwrap();
            let resource_content = std::str::from_utf8(&resource_data).unwrap_or_else(|_| {
                panic!(
                    "Unable to stringify resource data for field '{}' of {}",
                    field_name, element_name
                )
            });

            assert!(
                file_content.eq(&resource_content),
                "local '{field_name}' content differs from the one uploaded on IPFS for {}",
                element_name
            );
        } else {
            panic!(
                "The field '{field_name}' of {} is not a file (Should never happen !)",
                element_name
            );
        }
    } else {
        panic!("The '{field_name}' field is not an IPFS artifact for {}", element_name);
    }
}

/// Convert resource bytes to a ArtifactMetadata object.
///
/// # Arguments
///
/// * `raw_data` - resource data as bytes.
/// * `element_name` - name of the element linked to this resource.
///
/// # Returns
///
/// A [`ArtifactMetadata`] object.
fn resource_bytes_to_metadata(raw_data: &[u8], element_name: &String) -> ArtifactMetadata {
    let data = std::str::from_utf8(raw_data)
        .unwrap_or_else(|_| panic!("Unable to stringify raw metadata for {}", element_name));
    serde_json::from_str(data)
        .unwrap_or_else(|_| panic!("Unable to deserialize metadata for {}", element_name))
}

/// Convert resource bytes to a WorldMetadata object.
///
/// # Arguments
///
/// * `raw_data` - resource data as bytes.
/// * `element_name` - name of the element linked to this resource.
///
/// # Returns
///
/// A [`WorldMetadata`] object.
fn resource_bytes_to_world_metadata(raw_data: &[u8], element_name: &String) -> WorldMetadata {
    let data = std::str::from_utf8(raw_data)
        .unwrap_or_else(|_| panic!("Unable to stringify raw metadata for {}", element_name));
    serde_json::from_str(data)
        .unwrap_or_else(|_| panic!("Unable to deserialize metadata for {}", element_name))
}

/// Read the content of a resource identified by its IPFS URI.
///
/// # Arguments
///
/// * `client` - a IPFS client.
/// * `element_name` - the name of the element (model or contract) linked to this artifact.
/// * `uri` - the IPFS resource URI.
///
/// # Returns
///
/// A [`Vec<u8>`] containing the resource content as bytes.
async fn get_ipfs_resource_data(
    client: &HyperBackend,
    element_name: &String,
    uri: &String,
) -> Vec<u8> {
    let hash = get_hash_from_uri(uri);

    let res = client.cat(&hash).map_ok(|chunk| chunk.to_vec()).try_concat().await;
    assert!(res.is_ok(), "Unable to read the IPFS artifact {} for {}", uri, element_name);

    res.unwrap()
}

/// Check the validity of artifact metadata fields.
///
/// # Arguments
///
/// * `client` - a IPFS client.
/// * `metadata` - the metadata to check.
/// * `expected_metadata` - the metadata values coming from local Dojo metadata.
/// * `element_name` - the name of the element linked to this metadata.
async fn check_artifact_fields(
    client: &HyperBackend,
    metadata: &ArtifactMetadata,
    expected_metadata: &ArtifactMetadata,
    element_name: &String,
) {
    assert!(metadata.abi.is_some(), "'abi' field not set for {}", element_name);
    let abi = metadata.abi.as_ref().unwrap();
    let expected_abi = expected_metadata.abi.as_ref().unwrap();
    check_file_field(client, abi, expected_abi, "abi".to_string(), element_name).await;

    assert!(metadata.source.is_some(), "'source' field not set for {}", element_name);
    let source = metadata.source.as_ref().unwrap();
    let expected_source = expected_metadata.source.as_ref().unwrap();
    check_file_field(client, source, expected_source, "source".to_string(), element_name).await;
}

/// Check the validity of a IPFS artifact metadata.
///
/// # Arguments
///
/// * `client` - a IPFS client.
/// * `element_name` - the fully qualified name of the element linked to the artifact.
/// * `uri` - the full metadata URI.
/// * `expected_metadata` - the expected metadata values coming from local Dojo metadata.
async fn check_ipfs_metadata(
    client: &HyperBackend,
    element_name: &String,
    uri: &String,
    expected_metadata: &ArtifactMetadata,
) {
    let resource_bytes = get_ipfs_resource_data(client, element_name, uri).await;
    let metadata = resource_bytes_to_metadata(&resource_bytes, element_name);

    check_artifact_fields(client, &metadata, expected_metadata, element_name).await;
}

/// Rebuild the full metadata URI from an array of 3 FieldElement.
///
/// # Arguments
///
/// * `element_name` - name of the element (model or contract) linked to the metadata URI.
/// * `uri` - uri as an array of 3 FieldElement.
///
/// # Returns
///
/// A [`String`] containing the full metadata URI.
fn get_and_check_metadata_uri(element_name: &String, uri: &Vec<FieldElement>) -> String {
    assert!(uri.len() == 3, "bad metadata URI length for {} ({:#?})", element_name, uri);

    let mut i = 0;
    let mut full_uri = "".to_string();

    while i < uri.len() && uri[i] != FieldElement::ZERO {
        let uri_str = parse_cairo_short_string(&uri[i]);
        assert!(
            uri_str.is_ok(),
            "unable to parse the part {} of the metadata URI for {}",
            i + 1,
            element_name
        );

        full_uri = format!("{}{}", full_uri, uri_str.unwrap());

        i += 1;
    }

    assert!(!full_uri.is_empty(), "metadata URI is empty for {}", element_name);

    assert!(
        full_uri.starts_with("ipfs://"),
        "metadata URI for {} is not an IPFS artifact",
        element_name
    );

    full_uri
}

/// Check an artifact metadata read from the resource registry against its value
/// in the local Dojo metadata.
///
/// # Arguments
///
/// * `client` - a IPFS client.
/// * `world_reader` - a world reader object.
/// * `resource_id` - the resource ID in the resource registry.
/// * `element_name` - the fully qualified name of the element linked to this metadata.
/// * `dojo_metadata` - local Dojo metadata.
async fn check_artifact_metadata<P: starknet::providers::Provider + Sync>(
    client: &HyperBackend,
    world_reader: &WorldContractReader<P>,
    resource_id: FieldElement,
    element_name: &String,
    dojo_metadata: &DojoMetadata,
) {
    let resource = world_reader.metadata(&resource_id).call().await.unwrap();

    let expected_artifact = dojo_metadata.artifacts.get(element_name);
    assert!(
        expected_artifact.is_some(),
        "Unable to find local artifact metadata for {}",
        element_name
    );
    let expected_artifact = expected_artifact.unwrap();

    let full_uri = get_and_check_metadata_uri(element_name, &resource.metadata_uri);
    check_ipfs_metadata(client, element_name, &full_uri, expected_artifact).await;
}

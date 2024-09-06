#![allow(dead_code)]
use std::str::{self, FromStr};

use cainome::cairo_serde::ContractAddress;
use camino::Utf8Path;
use dojo_test_utils::migration::prepare_migration_with_world_and_seed;
use dojo_utils::TxnConfig;
use dojo_world::contracts::naming::{compute_bytearray_hash, compute_selector_from_tag};
use dojo_world::contracts::{WorldContract, WorldContractReader};
use dojo_world::manifest::{
    BaseManifest, DeploymentManifest, OverlayManifest, BASE_DIR, MANIFESTS_DIR, OVERLAYS_DIR,
    WORLD_CONTRACT_TAG,
};
use dojo_world::metadata::{
    dojo_metadata_from_workspace, get_default_namespace_from_ws, ArtifactMetadata, DojoMetadata,
    WorldMetadata, IPFS_CLIENT_URL, IPFS_PASSWORD, IPFS_USERNAME,
};
use dojo_world::migration::strategy::{prepare_for_migration, MigrationMetadata};
use dojo_world::migration::world::WorldDiff;
use dojo_world::uri::Uri;
use futures::TryStreamExt;
use ipfs_api_backend_hyper::{HyperBackend, IpfsApi, IpfsClient, TryFromUri};
use katana_runner::{KatanaRunner, KatanaRunnerConfig};
use starknet::core::types::{BlockId, BlockTag, Felt};
use starknet::macros::felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;

use crate::auth::ResourceType;
use crate::migration::{
    auto_authorize, execute_strategy, find_authorization_diff, upload_metadata,
};
use crate::test_utils::setup;
use crate::utils::get_contract_address_from_reader;

#[tokio::test(flavor = "multi_thread")]
async fn default_migrate_no_dry_run() {
    let config = setup::load_config();
    let ws = setup::setup_ws(&config);

    let sequencer = KatanaRunner::new_with_config(KatanaRunnerConfig {
        n_accounts: 10,
        dev: true,
        ..Default::default()
    })
    .expect("Fail to start runner");

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let _ = crate::migration::migrate(
        &ws,
        None,
        sequencer.url().to_string(),
        account,
        "dojo_examples",
        false,
        TxnConfig::init_wait(),
        None,
    )
    .await
    .is_ok();
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_auto_mine() {
    let config = setup::load_config();
    let ws = setup::setup_ws(&config);

    let (migration, _) = setup::setup_migration(&config, "dojo_examples").unwrap();

    let sequencer =
        KatanaRunner::new_with_config(KatanaRunnerConfig { n_accounts: 10, ..Default::default() })
            .expect("Fail to start runner");

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let declarers = setup::get_declarers_from_sequencer(&sequencer).await;

    execute_strategy(&ws, &migration, &account, TxnConfig::init_wait(), &declarers).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_block_time() {
    let config = setup::load_config();
    let ws = setup::setup_ws(&config);

    let (migration, _) = setup::setup_migration(&config, "dojo_examples").unwrap();

    let sequencer = KatanaRunner::new_with_config(KatanaRunnerConfig {
        n_accounts: 10,
        block_time: Some(1000),
        ..Default::default()
    })
    .expect("Fail to start runner");

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let declarers = setup::get_declarers_from_sequencer(&sequencer).await;

    execute_strategy(&ws, &migration, &account, TxnConfig::default(), &declarers).await.unwrap();
}

#[tokio::test]
async fn metadata_calculated_properly() {
    let config = setup::load_config();
    let ws = setup::setup_ws(&config);

    let base = config.manifest_path().parent().unwrap();
    let target_dir = format!("{}/target/dev", base);

    let profile_name = ws.current_profile().unwrap().to_string();

    let mut manifest = BaseManifest::load_from_path(
        &base.to_path_buf().join(MANIFESTS_DIR).join(&profile_name).join(BASE_DIR),
    )
    .unwrap();

    let overlay_dir = base.join(OVERLAYS_DIR).join(&profile_name);
    if overlay_dir.exists() {
        let overlay_manifest = OverlayManifest::load_from_path(&overlay_dir, &manifest).unwrap();
        manifest.merge(overlay_manifest);
    }

    let world = WorldDiff::compute(manifest, None, "dojo-test").unwrap();

    let migration = prepare_for_migration(
        None,
        felt!("0x12345"),
        &Utf8Path::new(&target_dir).to_path_buf(),
        world,
    )
    .unwrap();

    // verifies that key name and actual item name are same
    for (key, value) in migration.metadata.iter() {
        match value {
            MigrationMetadata::Contract(c) => {
                assert_eq!(key, &c.tag);
            }
        }
    }
}

#[tokio::test]
async fn migration_with_correct_calldata_second_time_work_as_expected() {
    let config = setup::load_config();
    let ws = setup::setup_ws(&config);

    let base = config.manifest_path().parent().unwrap();
    let target_dir = format!("{}/target/dev", base);

    let sequencer =
        KatanaRunner::new_with_config(KatanaRunnerConfig { n_accounts: 10, ..Default::default() })
            .expect("Failed to start runner.");

    let account = sequencer.account(0);

    let profile_name = ws.current_profile().unwrap().to_string();

    let mut manifest = BaseManifest::load_from_path(
        &base.to_path_buf().join(MANIFESTS_DIR).join(&profile_name).join(BASE_DIR),
    )
    .unwrap();

    let world = WorldDiff::compute(manifest.clone(), None, "dojo-test").unwrap();

    let migration = prepare_for_migration(
        None,
        felt!("0x12345"),
        &Utf8Path::new(&target_dir).to_path_buf(),
        world,
    )
    .unwrap();

    let declarers = setup::get_declarers_from_sequencer(&sequencer).await;

    let migration_output =
        execute_strategy(&ws, &migration, &account, TxnConfig::init_wait(), &declarers)
            .await
            .unwrap();

    let world_address = migration_output.world_address;

    let remote_manifest = DeploymentManifest::load_from_remote(sequencer.provider(), world_address)
        .await
        .expect("Failed to load remote manifest");

    let overlay_dir = base.join(OVERLAYS_DIR).join(profile_name);
    if overlay_dir.exists() {
        let overlay = OverlayManifest::load_from_path(&overlay_dir, &manifest)
            .expect("Failed to load overlay");

        // adding correct calldata
        manifest.merge(overlay);
    }
    let default_namespace = get_default_namespace_from_ws(&ws).unwrap();

    let world = WorldDiff::compute(manifest, Some(remote_manifest), &default_namespace)
        .expect("failed to update order");

    let migration = prepare_for_migration(
        Some(world_address),
        felt!("0x12345"),
        &Utf8Path::new(&target_dir).to_path_buf(),
        world,
    )
    .unwrap();

    let migration_output =
        execute_strategy(&ws, &migration, &account, TxnConfig::init_wait(), &declarers)
            .await
            .unwrap();

    assert!(migration_output.full);
}

#[tokio::test]
async fn migration_from_remote() {
    let config = setup::load_config();
    let ws = setup::setup_ws(&config);

    let base = config.manifest_path().parent().unwrap();
    let target_dir = format!("{}/target/dev", base);

    let sequencer =
        KatanaRunner::new_with_config(KatanaRunnerConfig { n_accounts: 10, ..Default::default() })
            .expect("Failed to start runner.");

    let account = sequencer.account(0);

    let profile_name = ws.current_profile().unwrap().to_string();

    let manifest = BaseManifest::load_from_path(
        &base.to_path_buf().join(MANIFESTS_DIR).join(&profile_name).join(BASE_DIR),
    )
    .unwrap();

    let world = WorldDiff::compute(manifest, None, "dojo-test").unwrap();

    let migration = prepare_for_migration(
        None,
        felt!("0x12345"),
        &Utf8Path::new(&target_dir).to_path_buf(),
        world,
    )
    .unwrap();

    let declarers = setup::get_declarers_from_sequencer(&sequencer).await;

    execute_strategy(&ws, &migration, &account, TxnConfig::init_wait(), &declarers).await.unwrap();

    let local_manifest = BaseManifest::load_from_path(
        &base.to_path_buf().join(MANIFESTS_DIR).join(&profile_name).join(BASE_DIR),
    )
    .unwrap();

    let remote_manifest = DeploymentManifest::load_from_remote(
        JsonRpcClient::new(HttpTransport::new(sequencer.url())),
        migration.world_address,
    )
    .await
    .unwrap();

    assert_eq!(local_manifest.world.inner.class_hash, remote_manifest.world.inner.class_hash);
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_metadata() {
    let config = setup::load_config();
    let ws = setup::setup_ws(&config);

    let (migration, _) = setup::setup_migration(&config, "dojo_examples").unwrap();

    let sequencer =
        KatanaRunner::new_with_config(KatanaRunnerConfig { n_accounts: 10, ..Default::default() })
            .expect("Failed to start runner.");

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let declarers = setup::get_declarers_from_sequencer(&sequencer).await;

    let output = execute_strategy(&ws, &migration, &account, TxnConfig::init_wait(), &declarers)
        .await
        .unwrap();

    let res = upload_metadata(&ws, &account, output.clone(), TxnConfig::init_wait()).await;
    assert!(res.is_ok());

    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(output.world_address, &provider);

    let client = IpfsClient::from_str(IPFS_CLIENT_URL)
        .unwrap_or_else(|_| panic!("Unable to initialize the IPFS Client"))
        .with_credentials(IPFS_USERNAME, IPFS_PASSWORD);

    let dojo_metadata =
        dojo_metadata_from_workspace(&ws).expect("No current package with dojo metadata found.");

    // check world metadata
    let resource = world_reader.metadata(&Felt::ZERO).call().await.unwrap();
    let element_name = WORLD_CONTRACT_TAG.to_string();

    let full_uri = resource.metadata_uri.to_string().unwrap();
    let resource_bytes = get_ipfs_resource_data(&client, &element_name, &full_uri).await;

    let metadata = resource_bytes_to_world_metadata(&resource_bytes, &element_name);

    assert_eq!(metadata.name, dojo_metadata.world.name, "");
    assert_eq!(metadata.description, dojo_metadata.world.description, "");
    assert_eq!(metadata.cover_uri, dojo_metadata.world.cover_uri, "");
    assert_eq!(metadata.icon_uri, dojo_metadata.world.icon_uri, "");
    assert_eq!(metadata.website, dojo_metadata.world.website, "");
    assert_eq!(metadata.socials, dojo_metadata.world.socials, "");

    // TODO: uncomment when https://github.com/dojoengine/dojo/issues/2137 is fixed.
    //     check_artifact_fields(
    // &client,
    // &metadata.artifacts,
    // &dojo_metadata.world.artifacts,
    // &element_name,
    // )
    // .await;
    // check model metadata
    //     for m in migration.models {
    // let selector = compute_selector_from_tag(&m.diff.tag);
    // check_artifact_metadata(&client, &world_reader, selector, &m.diff.tag, &dojo_metadata)
    // .await;
    // }
    // check contract metadata
    //     for c in migration.contracts {
    // let contract_address =
    // get_contract_address_from_reader(&world_reader, c.diff.tag.clone()).await.unwrap();
    //
    // check_artifact_metadata(
    // &client,
    // &world_reader,
    // contract_address,
    // &c.diff.tag,
    // &dojo_metadata,
    // )
    // .await;
    // }
}

#[tokio::test(flavor = "multi_thread")]
async fn migrate_with_auto_authorize() {
    let config = setup::load_config();
    let ws = setup::setup_ws(&config);

    let (migration, diff) = setup::setup_migration(&config, "dojo_examples").unwrap();

    let manifest_base = config.manifest_path().parent().unwrap();
    let mut manifest =
        BaseManifest::load_from_path(&manifest_base.join(MANIFESTS_DIR).join("dev").join(BASE_DIR))
            .unwrap();

    let overlay_dir = manifest_base.join(OVERLAYS_DIR).join("dev");
    if overlay_dir.exists() {
        let overlay_manifest = OverlayManifest::load_from_path(&overlay_dir, &manifest).unwrap();
        manifest.merge(overlay_manifest);
    }

    let sequencer =
        KatanaRunner::new_with_config(KatanaRunnerConfig { n_accounts: 10, ..Default::default() })
            .expect("Failed to start runner.");

    let mut account = sequencer.account(0);
    account.set_block_id(BlockId::Tag(BlockTag::Pending));

    let txn_config = TxnConfig::init_wait();

    let declarers = setup::get_declarers_from_sequencer(&sequencer).await;

    let output = execute_strategy(&ws, &migration, &account, txn_config, &declarers).await.unwrap();

    let world_address = migration.world_address;
    let world = WorldContract::new(world_address, account);

    let default_namespace = get_default_namespace_from_ws(&ws).unwrap();
    let (grant, revoke) =
        find_authorization_diff(&config.ui(), &world, &diff, Some(&output), &default_namespace)
            .await
            .unwrap();

    let res = auto_authorize(&ws, &world, &txn_config, &default_namespace, &grant, &revoke).await;
    assert!(res.is_ok());

    let provider = sequencer.provider();
    let world_reader = WorldContractReader::new(output.world_address, &provider);

    // check contract metadata
    for c in migration.contracts {
        let contract_address =
            get_contract_address_from_reader(&world_reader, c.diff.tag.clone()).await.unwrap();

        let contract = manifest.contracts.iter().find(|a| a.inner.tag == c.diff.tag).unwrap();

        for resource in &contract.inner.writes {
            let resource_type = ResourceType::from_str(resource).unwrap();

            let selector = match resource_type {
                ResourceType::Model(tag) => compute_selector_from_tag(&tag),
                ResourceType::Contract(tag) => compute_selector_from_tag(&tag),
                ResourceType::Namespace(ns) => compute_bytearray_hash(&ns),
                ResourceType::Selector(s) => s,
            };

            let contract_address = ContractAddress(contract_address);
            let is_writer =
                world_reader.is_writer(&selector, &contract_address).call().await.unwrap();
            assert!(is_writer);
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn migration_with_mismatching_world_address_and_seed() {
    let config = setup::load_config();
    let ws = setup::setup_ws(&config);

    let base_dir = config.manifest_path().parent().unwrap().to_path_buf();
    let target_dir = base_dir.join("target").join("dev");

    let default_namespace = get_default_namespace_from_ws(&ws).unwrap();

    let (strategy, _) = prepare_migration_with_world_and_seed(
        base_dir,
        target_dir,
        Some(Felt::ONE),
        "sozo_test",
        &default_namespace,
    )
    .unwrap();

    // The strategy.world has it's address set with the seed directly, and not
    // from the world address provided by the user.
    assert_ne!(strategy.world_address, strategy.world.unwrap().contract_address);
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
/// * `tag` - the tag of the element linked to this field.
async fn check_file_field(
    client: &HyperBackend,
    uri: &Uri,
    expected_uri: &Uri,
    field_name: String,
    tag: &String,
) {
    if let Uri::Ipfs(uri) = uri {
        let resource_data = get_ipfs_resource_data(client, tag, uri).await;
        assert!(!resource_data.is_empty(), "{field_name} IPFS artifact for {} is empty", tag);

        if let Uri::File(f) = expected_uri {
            let file_content = std::fs::read_to_string(f).unwrap();
            let resource_content = std::str::from_utf8(&resource_data).unwrap_or_else(|_| {
                panic!("Unable to stringify resource data for field '{}' of {}", field_name, tag)
            });

            assert!(
                file_content.eq(&resource_content),
                "local '{field_name}' content differs from the one uploaded on IPFS for {}",
                tag
            );
        } else {
            panic!("The field '{field_name}' of {} is not a file (Should never happen !)", tag);
        }
    } else {
        panic!("The '{field_name}' field is not an IPFS artifact for {}", tag);
    }
}

/// Convert resource bytes to a ArtifactMetadata object.
///
/// # Arguments
///
/// * `raw_data` - resource data as bytes.
/// * `tag` - tag of the element linked to this resource.
///
/// # Returns
///
/// A [`ArtifactMetadata`] object.
fn resource_bytes_to_metadata(raw_data: &[u8], tag: &String) -> ArtifactMetadata {
    let data = std::str::from_utf8(raw_data)
        .unwrap_or_else(|_| panic!("Unable to stringify raw metadata for {}", tag));
    serde_json::from_str(data)
        .unwrap_or_else(|_| panic!("Unable to deserialize metadata for {}", tag))
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
/// * `tag` - the tag of the element (model or contract) linked to this artifact.
/// * `uri` - the IPFS resource URI.
///
/// # Returns
///
/// A [`Vec<u8>`] containing the resource content as bytes.
async fn get_ipfs_resource_data(client: &HyperBackend, tag: &String, uri: &String) -> Vec<u8> {
    let hash = get_hash_from_uri(uri);

    let res = client.cat(&hash).map_ok(|chunk| chunk.to_vec()).try_concat().await;
    assert!(res.is_ok(), "Unable to read the IPFS artifact {} for {}", uri, tag);

    res.unwrap()
}

/// Check the validity of artifact metadata fields.
///
/// # Arguments
///
/// * `client` - a IPFS client.
/// * `metadata` - the metadata to check.
/// * `expected_metadata` - the metadata values coming from local Dojo metadata.
/// * `tag` - the tag of the element linked to this metadata.
async fn check_artifact_fields(
    client: &HyperBackend,
    metadata: &ArtifactMetadata,
    expected_metadata: &ArtifactMetadata,
    tag: &String,
) {
    println!("metadata {:?}", metadata);
    println!("expected_metadata {:?}", expected_metadata);

    assert!(metadata.abi.is_some(), "'abi' field not set for {}", tag);
    let abi = metadata.abi.as_ref().unwrap();
    let expected_abi = expected_metadata.abi.as_ref().unwrap();
    check_file_field(client, abi, expected_abi, "abi".to_string(), tag).await;

    // For now source are not expended, uncomment when https://github.com/dojoengine/dojo/issues/2137 is fixed.
    // assert!(metadata.source.is_some(), "'source' field not set for {}", tag);
    // let source = metadata.source.as_ref().unwrap();
    // let expected_source = expected_metadata.source.as_ref().unwrap();
    // check_file_field(client, source, expected_source, "source".to_string(), tag).await;
}

/// Check the validity of a IPFS artifact metadata.
///
/// # Arguments
///
/// * `client` - a IPFS client.
/// * `tag` - the tag of the element linked to the artifact.
/// * `uri` - the full metadata URI.
/// * `expected_metadata` - the expected metadata values coming from local Dojo metadata.
async fn check_ipfs_metadata(
    client: &HyperBackend,
    tag: &String,
    uri: &String,
    expected_metadata: &ArtifactMetadata,
) {
    let resource_bytes = get_ipfs_resource_data(client, tag, uri).await;
    let metadata = resource_bytes_to_metadata(&resource_bytes, tag);

    check_artifact_fields(client, &metadata, expected_metadata, tag).await;
}

/// Check an artifact metadata read from the resource registry against its value
/// in the local Dojo metadata.
///
/// # Arguments
///
/// * `client` - a IPFS client.
/// * `world_reader` - a world reader object.
/// * `resource_id` - the resource ID in the resource registry.
/// * `tag` - the tag of the element linked to this metadata.
/// * `dojo_metadata` - local Dojo metadata.
async fn check_artifact_metadata<P: starknet::providers::Provider + Sync>(
    client: &HyperBackend,
    world_reader: &WorldContractReader<P>,
    resource_id: Felt,
    tag: &String,
    dojo_metadata: &DojoMetadata,
) {
    let resource = world_reader.metadata(&resource_id).call().await.unwrap();

    let expected_resource = dojo_metadata.resources_artifacts.get(tag);
    assert!(expected_resource.is_some(), "Unable to find local artifact metadata for {}", tag);
    let expected_resource = expected_resource.unwrap();

    check_ipfs_metadata(
        client,
        tag,
        &resource.metadata_uri.to_string().unwrap(),
        &expected_resource.artifacts,
    )
    .await;
}

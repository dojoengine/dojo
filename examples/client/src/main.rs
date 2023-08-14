//! This example demonstrates how to use the `WorldPartialSyncer` to sync a single entity from the
//! world contract. The entity is synced to an in-memory storage, and then the storage is queried
//! for the entity's component.
//!
//! This uses the example project under `examples/ecs`.
//!
//! To run this example, you must first migrate the `ecs` example project with `--name ohayo`

use std::sync::Arc;
use std::time::Duration;
use std::vec;

use async_std::sync::RwLock;
use dojo_client::contract::world::{WorldContract, WorldContractReader};
use dojo_client::storage::EntityStorage;
use dojo_client::sync::{EntityComponentReq, WorldPartialSyncer};
use starknet::accounts::SingleOwnerAccount;
use starknet::core::types::FieldElement;
use starknet::core::utils::cairo_short_string_to_felt;
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::{JsonRpcClient, Provider};
use starknet::signers::{LocalWallet, SigningKey};
use storage::InMemoryStorage;
use tokio::select;
use url::Url;

mod storage;

async fn run_execute_system_loop() {
    let world_address = FieldElement::from_hex_be(
        "0x2886968c76e33e66d2206ad75f57c931bafed9b784294d3649a950e0cd3e973",
    )
    .unwrap();

    let provider =
        JsonRpcClient::new(HttpTransport::new(Url::parse("http://localhost:5050").unwrap()));

    let chain_id = provider.chain_id().await.unwrap();

    let account = SingleOwnerAccount::new(
        provider,
        LocalWallet::from_signing_key(SigningKey::from_secret_scalar(
            FieldElement::from_hex_be(
                "0x0300001800000000300000180000000000030000000000003006001800006600",
            )
            .unwrap(),
        )),
        FieldElement::from_hex_be(
            "0x03ee9e18edc71a6df30ac3aca2e0b02a198fbce19b7480a63a0d71cbd76652e0",
        )
        .unwrap(),
        chain_id,
    );

    let world = WorldContract::new(world_address, &account);

    loop {
        println!("Execute spawn");
        let _ = world.execute("spawn", vec![]).await.unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn run_read_storage_loop(storage: Arc<RwLock<InMemoryStorage>>, keys: Vec<FieldElement>) {
    loop {
        // Read the entity's position directly from the storage.
        let values = storage
            .read()
            .await
            .get(cairo_short_string_to_felt("Position").unwrap(), keys.clone(), 2)
            .await
            .unwrap();

        println!("Storage: Position x {:#x} y {:#x}", values[0], values[1]);
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[tokio::main]
async fn main() {
    let client =
        JsonRpcClient::new(HttpTransport::new(Url::parse("http://localhost:5050").unwrap()));

    let world_address = FieldElement::from_hex_be(
        "0x2886968c76e33e66d2206ad75f57c931bafed9b784294d3649a950e0cd3e973",
    )
    .unwrap();
    let keys: Vec<FieldElement> = vec![
        FieldElement::from_hex_be(
            "0x3ee9e18edc71a6df30ac3aca2e0b02a198fbce19b7480a63a0d71cbd76652e0",
        )
        .unwrap(),
    ];

    let storage = Arc::new(RwLock::new(InMemoryStorage::new()));

    let world_reader = WorldContractReader::new(world_address, &client);
    let mut syncer = WorldPartialSyncer::new(
        Arc::clone(&storage),
        &world_reader,
        vec![EntityComponentReq { component: String::from("Position"), keys: keys.clone() }],
    )
    .with_interval(2000);

    select! {
        _ = run_execute_system_loop() => {}
        _ = syncer.start() => {}
        _ = run_read_storage_loop(storage, keys) => {}
    }
}

use std::env::current_dir;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use dojo_project::migration::world::World;
use starknet::accounts::SingleOwnerAccount;
use starknet::core::chain_id;
use starknet::core::types::FieldElement;
use starknet::providers::SequencerGatewayProvider;
use starknet::signers::{LocalWallet, SigningKey};
use url::Url;

#[derive(Args)]
pub struct MigrateArgs {
    #[clap(help = "Source directory")]
    path: Option<Utf8PathBuf>,

    #[clap(short, long, help = "Perform a dry run and outputs the plan to be executed")]
    plan: bool,
}

#[tokio::main]
pub async fn run(args: MigrateArgs) -> Result<()> {
    let source_dir = match args.path {
        Some(path) => {
            if path.is_absolute() {
                path
            } else {
                let mut current_path = current_dir().unwrap();
                current_path.push(path);
                Utf8PathBuf::from_path_buf(current_path).unwrap()
            }
        }
        None => Utf8PathBuf::from_path_buf(current_dir().unwrap()).unwrap(),
    };

    let world = World::from_path(source_dir.clone()).await?;
    let mut migration = world.prepare_for_migration(source_dir);

    let provider = SequencerGatewayProvider::new(
        Url::parse("http://127.0.0.1:5050/gateway").unwrap(),
        Url::parse("http://127.0.0.1:5050/feeder_gateway").unwrap(),
    );

    let signer = LocalWallet::from(SigningKey::from_secret_scalar(
        FieldElement::from_hex_be("0x5d4fb5e2c807cd78ac51675e06be7099").unwrap(),
    ));
    let address = FieldElement::from_hex_be(
        "0x5f6fd2a43f4bce1bdfb2d0e9212d910227d9f67cf1425f2a9ceae231572c643",
    )
    .unwrap();
    let account = SingleOwnerAccount::new(provider, signer, address, chain_id::TESTNET);

    migration.execute(account).await?;

    Ok(())
}

#[tokio::test]
async fn test_migrate() {
    use dojo_test_utils::devnet;

    devnet::start_devnet_and_wait().unwrap_or_else(|err| {
        panic!(
            "Failed to start devnet: {}. Make sure you have the devnet running on port 5050",
            err
        )
    });

    let source_dir = Utf8PathBuf::from_path_buf("../../examples".into()).unwrap();
    let world = World::from_path(source_dir.clone())
        .await
        .unwrap_or_else(|err| panic!("Failed to load world from path: {}.", err));
    let mut migration = world.prepare_for_migration(source_dir);

    let provider = SequencerGatewayProvider::new(
        Url::parse("http://127.0.0.1:5050/gateway").unwrap(),
        Url::parse("http://127.0.0.1:5050/feeder_gateway").unwrap(),
    );

    let signer = LocalWallet::from(SigningKey::from_secret_scalar(
        FieldElement::from_hex_be("0x5d4fb5e2c807cd78ac51675e06be7099").unwrap(),
    ));
    let address = FieldElement::from_hex_be(
        "0x5f6fd2a43f4bce1bdfb2d0e9212d910227d9f67cf1425f2a9ceae231572c643",
    )
    .unwrap();
    let account = SingleOwnerAccount::new(provider, signer, address, chain_id::TESTNET);

    migration
        .execute(account)
        .await
        .unwrap_or_else(|op| panic!("Failed to execute migration: {}.", op));
}

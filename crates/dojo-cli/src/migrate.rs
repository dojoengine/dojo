use std::env::current_dir;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use dojo_project::migration::world::World;
use dojo_signers::FromEnv;
use dotenv::dotenv;
use starknet::accounts::SingleOwnerAccount;
use starknet::core::chain_id;
use starknet::core::types::FieldElement;
use starknet::providers::SequencerGatewayProvider;
use starknet::signers::LocalWallet;

#[derive(Args)]
pub struct MigrateArgs {
    #[clap(help = "Source directory")]
    path: Option<Utf8PathBuf>,

    #[clap(short, long, help = "Perform a dry run and outputs the plan to be executed")]
    plan: bool,
}

#[tokio::main]
pub async fn run(args: MigrateArgs) -> Result<()> {
    dotenv().ok();

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
    let mut migration = world.prepare_for_migration(source_dir)?;

    let provider = SequencerGatewayProvider::starknet_alpha_goerli();
    let signer = LocalWallet::from_env()?;
    let address = FieldElement::from_hex_be(
        "0x03cD4f9b4bd4D5eF087012D228E9ee6761eE10d02Bd23bed6055BF6799DD98b8",
    )
    .unwrap();
    let account = SingleOwnerAccount::new(provider, signer, address, chain_id::TESTNET);

    migration.execute(account).await?;

    Ok(())
}

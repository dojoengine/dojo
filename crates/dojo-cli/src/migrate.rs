use std::env::current_dir;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use dojo_project::migration::world::World;
use starknet::providers::SequencerGatewayProvider;
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

    migration.execute(provider).await?;

    Ok(())
}

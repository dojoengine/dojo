use std::env::current_dir;

use anyhow::Result;
use camino::Utf8PathBuf;
use clap::Args;
use dojo_project::migration::World;

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

    let world = World::from_path(source_dir).await?;

    unimplemented!("dojo migrate");
}

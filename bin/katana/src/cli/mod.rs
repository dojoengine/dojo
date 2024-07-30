mod node;

use anyhow::Result;
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser)]
#[command(name = "katana", author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    commands: Option<Commands>,

    #[command(flatten)]
    node: node::NodeArgs,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        if let Some(cmd) = self.commands {
            return match cmd {
                Commands::Completions(args) => args.execute(),
            };
        }

        self.node.execute()
    }
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "Generate shell completion file for specified shell")]
    Completions(CompletionsArgs),
}

#[derive(Debug, Args)]
struct CompletionsArgs {
    shell: Shell,
}

impl CompletionsArgs {
    fn execute(self) -> Result<()> {
        let mut command = Cli::command();
        let name = command.get_name().to_string();
        clap_complete::generate(self.shell, &mut command, name, &mut std::io::stdout());
        Ok(())
    }
}

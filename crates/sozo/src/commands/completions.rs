use std::io;

use crate::SozoArgs;
use anyhow::Result;
use clap::{Args, CommandFactory};
use clap_complete::{generate, Shell};

#[derive(Args, Debug)]
pub struct CompletionsArgs {
    shell: Shell,
}

impl CompletionsArgs {
    pub fn run(self) -> Result<()> {
        let mut command = SozoArgs::command();
        let name = command.get_name().to_string();
        generate(self.shell, &mut command, name, &mut io::stdout());
        Ok(())
    }
}

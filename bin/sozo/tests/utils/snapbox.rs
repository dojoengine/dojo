use std::process::Command as StdCommand;

use snapbox::cmd::{cargo_bin, Command as SnapboxCommand};

pub fn get_snapbox() -> SnapboxCommand {
    SnapboxCommand::from_std(std())
}

fn std() -> StdCommand {
    StdCommand::new(cargo_bin("sozo"))
}

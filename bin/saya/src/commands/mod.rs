use std::process::{Command, Output, Stdio};

pub mod build;
pub mod migrate;

pub use build::BuildCommandSetup;
pub use migrate::MigrateComandSetup;
pub trait CommandUtil {
    fn inherit_io_wait_with_output(&mut self) -> Output;
}

impl CommandUtil for Command {
    fn inherit_io_wait_with_output(&mut self) -> Output {
        self.stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap()
            .wait_with_output()
            .unwrap()
    }
}

pub trait OutputUtil {
    fn unwrap(&self);
}

impl OutputUtil for std::process::ExitStatus {
    fn unwrap(&self) {
        if !self.success() {
            eprintln!("Command failed: {:?}", self);
            std::process::exit(1);
        }
    }
}

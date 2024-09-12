use std::{path::PathBuf, process::Command};

pub struct BuildCommandSetup {
    saya_manifest_path: PathBuf,
}

impl BuildCommandSetup {
    pub fn new(saya_manifest_path: impl Into<PathBuf>) -> Self {
        Self { saya_manifest_path: saya_manifest_path.into() }
    }
    pub fn command(&self) -> Command {
        let mut c = Command::new("cargo");
        c.arg("run")
            .arg("-r")
            .arg("--bin")
            .arg("sozo")
            .arg("--")
            .arg("build")
            .arg("--manifest-path")
            .arg(&self.saya_manifest_path);
        c
    }
}

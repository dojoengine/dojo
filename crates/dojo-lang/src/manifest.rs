use std::collections::HashMap;
use std::fs::{self, File};
use std::path::Path;

use smol_str::SmolStr;

#[cfg(test)]
#[path = "manifest_test.rs"]
mod test;

#[derive(Default)]
pub(crate) struct Manifest(dojo_world::manifest::Manifest);

impl Manifest {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let file = fs::OpenOptions::new().read(true).open(path)?;
        Ok(Self::try_from(file)?)
    }

    pub fn write_to_file(&self, path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        let file = fs::OpenOptions::new().write(true).truncate(true).create(true).open(path)?;
        serde_json::to_writer_pretty(file, &self.0)?;
        Ok(())
    }
}

impl TryFrom<File> for Manifest {
    type Error = serde_json::Error;
    fn try_from(file: File) -> Result<Self, Self::Error> {
        let buffer = std::io::BufReader::new(&file);
        Ok(Self(serde_json::from_reader(buffer).unwrap_or_default()))
    }
}

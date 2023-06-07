use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

use super::StarknetWrapper;

pub trait SerializableState {
    fn dump_state(&self, path: &PathBuf) -> std::io::Result<()>;

    fn load_state(&mut self, path: &PathBuf) -> std::io::Result<()>;
}

impl SerializableState for StarknetWrapper {
    fn dump_state(&self, path: &PathBuf) -> std::io::Result<()> {
        let encoded: Vec<u8> = bincode::serialize(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&encoded)?;
        Ok(())
    }

    fn load_state(&mut self, path: &PathBuf) -> std::io::Result<()> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        let mut buffer = Vec::new();
        reader.read_to_end(&mut buffer)?;

        // decode buffer content
        let decoded: StarknetWrapper = bincode::deserialize(&buffer)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        *self = decoded;
        Ok(())
    }
}

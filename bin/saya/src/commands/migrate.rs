use std::path::PathBuf;
use std::process::Command;

use regex::Regex;
use serde::{Deserialize, Serialize};
use starknet_crypto::Felt;

use crate::l2::{Account, L2};

pub struct MigrateComandSetup {
    manifest_path: PathBuf,
    l2_endpoint: String,
    l2_private_key: String,
    l2_account_address: String,
}

impl MigrateComandSetup {
    pub fn new(saya_manifest_path: impl Into<PathBuf>, l2: &L2) -> Self {
        let Account { address, private_key } = l2.account();
        Self {
            manifest_path: saya_manifest_path.into(),
            l2_endpoint: l2.endpoint(),
            l2_private_key: private_key.to_hex_string(),
            l2_account_address: address.to_hex_string(),
        }
    }

    pub fn command(&self) -> MigrateCommand {
        let mut c = Command::new("cargo");
        c.arg("run")
            .arg("-r")
            .arg("--bin")
            .arg("sozo")
            .arg("--")
            .arg("migrate")
            .arg("apply")
            .arg("--manifest-path")
            .arg(&self.manifest_path)
            .arg("--rpc-url")
            .arg(&self.l2_endpoint)
            .arg("--private-key")
            .arg(&self.l2_private_key)
            .arg("--account-address")
            .arg(&self.l2_account_address);
        MigrateCommand(c)
    }
}

pub struct MigrateCommand(Command);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub block_number: u64,
    pub world_address: Felt,
}

impl MigrateCommand {
    pub fn wait_get_unwrap(&mut self) -> World {
        let c = self
            .0
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .unwrap()
            .wait_with_output()
            .unwrap();
        let output_string = String::from_utf8(c.stdout).unwrap();
        println!("{output_string}");
        let world_address_re =
            Regex::new(r"World\s+on\s+block\s+#(\d+)\s+at\s+address\s+(0x[0-9a-fA-F]+)").unwrap();
        // Find the first match for the World address and block number
        let captures = world_address_re.captures(&output_string).unwrap();
        World {
            block_number: captures[1].parse().unwrap(),
            world_address: Felt::from_hex(&captures[2]).unwrap(),
        }
    }
}

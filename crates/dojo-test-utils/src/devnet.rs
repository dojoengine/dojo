use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use reqwest::StatusCode;

pub fn start_devnet_and_wait() -> Result<(), Box<dyn std::error::Error>> {
    let devnet_command = "starknet-devnet";
    let args = [
        "--cairo-compiler-manifest",
        "./cairo/Cargo.toml",
        "--seed",
        "420",
        "--disable-rpc-request-validation",
    ];

    Command::new(devnet_command).args(args).spawn().expect("Failed to spawn devnet process");

    let client = Client::new();
    let poll_interval = Duration::from_secs(1);
    let url = "http://127.0.0.1:5050/";
    let timeout = Duration::from_secs(30);
    let deadline = Instant::now() + timeout;

    loop {
        let response = client.get(url).send();

        if let Ok(res) = response {
            if res.status() == StatusCode::OK {
                break;
            }
        }

        if Instant::now() >= deadline {
            return Err("Timeout waiting for devnet to be alive".into());
        }

        thread::sleep(poll_interval);
    }

    Ok(())
}

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use starknet::providers::jsonrpc::HttpTransport;
use starknet::providers::JsonRpcClient;
#[cfg(test)]
use starknet::providers::Provider;
use url::Url;

use runner_macro::katana_test;

#[derive(Debug)]
pub struct KatanaRunner {
    child: Child,
}

fn find_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() // This might need to me mutexed
}

impl KatanaRunner {
    pub fn new() -> Result<(Self, JsonRpcClient<HttpTransport>)> {
        Self::new_with_port(find_free_port())
    }

    pub fn new_from_macro(_name: &str, port: u16) -> Result<(Self, JsonRpcClient<HttpTransport>)> {
        Self::new_with_port(port)
    }

    pub fn new_with_port(port: u16) -> Result<(Self, JsonRpcClient<HttpTransport>)> {
        let mut temp_dir = std::env::temp_dir();
        temp_dir.push("dojo");
        temp_dir.push("logs");
        temp_dir.push(format!("katana-{}.log", port));

        eprintln!("Writing katana logs to {}", temp_dir.to_str().unwrap());

        let mut child = Command::new("katana")
            .args(["-p", &port.to_string()])
            .args(["--json-log"])
            .stdout(Stdio::piped())
            .spawn()
            .context("failed to start subprocess")?;

        let stdout = child.stdout.take().context("failed to take subprocess stdout")?;

        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            KatanaRunner::wait_for_server_started_and_signal(temp_dir.as_path(), stdout, sender);
        });

        receiver
            .recv_timeout(Duration::from_secs(5))
            .context("timeout waiting for server to start")?;

        let url =
            Url::parse(&format!("http://127.0.0.1:{}/", port)).context("Failed to parse url")?;
        let provider = JsonRpcClient::new(HttpTransport::new(url));

        Ok((KatanaRunner { child }, provider))
    }

    fn wait_for_server_started_and_signal(path: &Path, stdout: ChildStdout, sender: Sender<()>) {
        let reader = BufReader::new(stdout);

        if let Some(dir_path) = path.parent() {
            if !dir_path.exists() {
                fs::create_dir_all(dir_path).unwrap();
            }
        }
        let mut log_writer = File::create(path).expect("failed to create log file");

        for line in reader.lines() {
            let line = line.expect("failed to read line from subprocess stdout");
            writeln!(log_writer, "{}", line).expect("failed to write to log file");

            if line.contains(r#""target":"katana""#) {
                sender.send(()).expect("failed to send start signal");
            }
        }
    }
}

impl Drop for KatanaRunner {
    fn drop(&mut self) {
        if let Err(e) = self.child.kill() {
            eprintln!("Failed to kill katana subprocess: {}", e);
        }
        if let Err(e) = self.child.wait() {
            eprintln!("Failed to wait for katana subprocess: {}", e);
        }
    }
}

#[katana_test]
async fn test_run() {
    for _ in 0..10 {
        let (_katana_guard, provider) =
            KatanaRunner::new().expect("failed to start another katana");

        let _block_number = provider.block_number().await.unwrap();
        // created by the macro at the beginning of the test
        let _other_block_number = katana_provider.block_number().await.unwrap();
    }
}

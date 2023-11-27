use anyhow::{Context, Result};
use starknet::providers::{jsonrpc::HttpTransport, JsonRpcClient};
use std::{
    fs::{self, File},
    io::Write,
    io::{BufRead, BufReader},
    net::TcpListener,
    path::Path,
    process::{Child, ChildStdout, Command, Stdio},
    sync::mpsc::{self, Sender},
    thread,
    time::Duration,
};
use url::Url;

use crate::{KatanaRunnerBuilder, KatanaRunnerConfig};

#[derive(Debug)]
pub struct KatanaBinary {
    child: Child,
}

pub fn find_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() // This might need to me mutexed
}

impl KatanaBinary {
    pub fn build() -> KatanaRunnerBuilder {
        KatanaRunnerBuilder::new()
    }

    pub fn new(config: KatanaRunnerConfig) -> Result<(Self, JsonRpcClient<HttpTransport>)> {
        let katana_path = "katana";
        let port = config.port.unwrap_or(find_free_port());
        let log_filename = format!("logs/katana-{}.log", port);

        let mut child = Command::new(katana_path)
            .args(["-p", &port.to_string()])
            .args(["--json-log"])
            .stdout(Stdio::piped())
            .spawn()
            .context("failed to start subprocess")?;

        let stdout = child.stdout.take().context("failed to take subprocess stdout")?;

        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            KatanaBinary::wait_for_server_started_and_signal(
                Path::new(&log_filename),
                stdout,
                sender,
            );
        });

        receiver
            .recv_timeout(Duration::from_secs(5))
            .context("timeout waiting for server to start")?;

        let url =
            Url::parse(&format!("http://127.0.0.1:{}/", port)).context("Failed to parse url")?;
        let provider = JsonRpcClient::new(HttpTransport::new(url));

        Ok((KatanaBinary { child }, provider))
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

impl Drop for KatanaBinary {
    fn drop(&mut self) {
        if let Err(e) = self.child.kill() {
            eprintln!("Failed to kill katana subprocess: {}", e);
        }
        if let Err(e) = self.child.wait() {
            eprintln!("Failed to wait for katana subprocess: {}", e);
        }
    }
}

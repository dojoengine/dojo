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

#[derive(Debug)]
pub struct KatanaRunner {
    child: Child,
    client: JsonRpcClient<HttpTransport>,
}

pub fn find_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() // This might need to me mutexed
}

impl KatanaRunner {
    pub fn new() -> Self {
        let katana_path = "katana";
        let port = find_free_port();
        let log_filename = format!("logs/katana-{}.log", port);

        let mut child = Command::new(katana_path)
            .args(["-p", &port.to_string()])
            .args(["--json-log"])
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to start subprocess");

        let stdout = child.stdout.take().expect("failed to take subprocess stdout");

        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            KatanaRunner::wait_for_server_started_and_signal(
                Path::new(&log_filename),
                stdout,
                sender,
            );
        });

        receiver.recv_timeout(Duration::from_secs(5)).expect("timeout waiting for server to start");

        let client = JsonRpcClient::new(HttpTransport::new(
            Url::parse(&format!("http://0.0.0.0:{}/", port)).unwrap(),
        ));

        KatanaRunner { child, client }
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

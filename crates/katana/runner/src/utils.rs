use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::ChildStdout;

pub fn find_free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port() // This might need to me mutexed
}

pub fn listen_to_stdout(log_file: &Path, stdout: ChildStdout) {
    let reader = BufReader::new(stdout);

    if let Some(dir_path) = log_file.parent() {
        if !dir_path.exists() {
            fs::create_dir_all(dir_path).unwrap();
        }
    }
    let mut log_writer = File::create(log_file).expect("failed to create log file");

    for line in reader.lines() {
        let line = line.expect("failed to read line from subprocess stdout");
        writeln!(log_writer, "{}", line).expect("failed to write to log file");
    }
}

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Duration;

use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use crate::KatanaRunner;

#[derive(Serialize, Deserialize)]
pub struct TimedLog<T> {
    timestamp: String,
    level: String,
    fields: T,
    target: String,
}

#[derive(Serialize, Deserialize)]
pub struct Message {
    message: String,
}

pub type Log = TimedLog<Message>;

impl KatanaRunner {
    pub fn blocks(&self) -> Vec<Log> {
        BufReader::new(File::open(&self.log_filename).unwrap())
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| match serde_json::from_str(&line) {
                Ok(log) => Some(log),
                Err(_) => None,
            })
            .filter_map(|log: Log| match log.fields.message.contains("⛏️ Block") {
                true => Some(log),
                false => None,
            })
            .collect()
    }

    pub async fn blocks_until_empty(&self) -> Vec<Log> {
        let mut blocks = self.blocks();
        loop {
            if let Some(block) = blocks.last() {
                if block.fields.message.contains("mined with 0 transactions") {
                    break;
                }
            }

            let len_at_call = blocks.len();
            while len_at_call == blocks.len() {
                sleep(Duration::from_secs(1)).await;
                blocks = self.blocks();
            }
        }
        blocks
    }

    pub async fn block_sizes(&self) -> Vec<u32> {
        self.blocks_until_empty()
            .await
            .into_iter()
            .map(|block| {
                let limit = block
                    .fields
                    .message
                    .find(" transactions")
                    .expect("Failed to find transactions in block");
                let number = block.fields.message[..limit].split(' ').last().unwrap();
                number.parse::<u32>().expect("Failed to parse number of transactions")
            })
            .collect()
    }

    pub async fn block_times(&self) -> Vec<i64> {
        let mut v = self
            .blocks_until_empty()
            .await
            .into_iter()
            .map(|block| block.timestamp.parse().expect("Failed to parse time"))
            .collect::<Vec<DateTime<Utc>>>()
            .windows(2)
            .map(|w| (w[1] - w[0]).num_milliseconds())
            .collect::<Vec<_>>();

        // First block has no previous one, so always has a time of 0
        v.insert(0, 0);
        v
    }

    pub async fn steps(&self) -> Vec<u64> {
        let matching = "Transaction resource usage: Steps: ";
        BufReader::new(File::open(&self.log_filename).unwrap())
            .lines()
            .filter_map(|line| {
                let line = line.unwrap();
                if let Some(start) = line.find(matching) {
                    let end = line.find(" | ");
                    let steps = line[start + matching.len()..end.unwrap()].to_string();

                    Some(steps.parse::<u64>().unwrap())
                } else {
                    None
                }
            })
            .collect()
    }
}

#[test]
fn test_parse_katana_logs() {
    let log = r#"{"timestamp":"2024-01-24T15:59:50.793948Z","level":"INFO","fields":{"message":"⛏️ Block 45 mined with 0 transactions"},"target":"backend"}"#;
    let log: Log = serde_json::from_str(log).unwrap();
    assert_eq!(log.fields.message, "⛏️ Block 45 mined with 0 transactions");
}

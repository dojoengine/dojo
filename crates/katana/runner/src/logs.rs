use chrono::prelude::*;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::Duration;
use tokio::time::sleep;

use crate::KatanaRunner;

impl KatanaRunner {
    pub fn blocks(&self) -> Vec<String> {
        BufReader::new(File::open(&self.log_filename).unwrap())
            .lines()
            .filter_map(|line| {
                let line = line.unwrap();
                if line.contains("⛏️ Block") {
                    Some(line)
                } else {
                    None
                }
            })
            .collect()
    }

    pub async fn blocks_until_empty(&self) -> Vec<String> {
        let mut blocks = self.blocks();
        loop {
            if let Some(block) = blocks.last() {
                if block.contains("mined with 0 transactions") {
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
                let limit =
                    block.find(" transactions").expect("Failed to find transactions in block");
                let number = block[..limit].split(' ').last().unwrap();
                number.parse::<u32>().expect("Failed to parse number of transactions")
            })
            .collect()
    }

    pub async fn block_times(&self) -> Vec<i64> {
        self.blocks_until_empty()
            .await
            .into_iter()
            .map(|block| {
                let time = block.split('"').nth(3).unwrap();
                let time: DateTime<Utc> = time.parse().expect("Failed to parse time");
                time
            })
            .collect::<Vec<_>>()
            .windows(2)
            .map(|w| (w[1] - w[0]).num_milliseconds())
            .collect()
    }
}

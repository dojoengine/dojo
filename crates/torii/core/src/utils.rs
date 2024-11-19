use std::time::Duration;

use anyhow::Result;
use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use tokio_util::bytes::Bytes;
use tracing::info;

use crate::constants::{
    IPFS_CLIENT_MAX_RETRY, IPFS_CLIENT_PASSWORD, IPFS_CLIENT_URL, IPFS_CLIENT_USERNAME,
};

pub fn must_utc_datetime_from_timestamp(timestamp: u64) -> DateTime<Utc> {
    let naive_dt = DateTime::from_timestamp(timestamp as i64, 0)
        .expect("Failed to convert timestamp to NaiveDateTime");
    naive_dt.to_utc()
}

pub fn utc_dt_string_from_timestamp(timestamp: u64) -> String {
    must_utc_datetime_from_timestamp(timestamp).to_rfc3339()
}

pub async fn fetch_content_from_ipfs(cid: &str, mut retries: u8) -> Result<Bytes> {
    let client = IpfsClient::from_str(IPFS_CLIENT_URL)?
        .with_credentials(IPFS_CLIENT_USERNAME, IPFS_CLIENT_PASSWORD);
    while retries > 0 {
        let response = client.cat(cid).map_ok(|chunk| chunk.to_vec()).try_concat().await;
        match response {
            Ok(stream) => return Ok(Bytes::from(stream)),
            Err(e) => {
                retries -= 1;
                if retries > 0 {
                    info!(
                        error = %e,
                        "Fetch uri."
                    );
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    }

    Err(anyhow::anyhow!(format!(
        "Failed to pull data from IPFS after {} attempts, cid: {}",
        IPFS_CLIENT_MAX_RETRY, cid
    )))
}
// tests
#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, NaiveTime, Utc};

    use super::*;

    #[test]
    fn test_must_utc_datetime_from_timestamp() {
        let timestamp = 1633027200;
        let expected_date = NaiveDate::from_ymd_opt(2021, 9, 30).unwrap();
        let expected_time = NaiveTime::from_hms_opt(18, 40, 0).unwrap();
        let expected =
            DateTime::<Utc>::from_naive_utc_and_offset(expected_date.and_time(expected_time), Utc);
        let out = must_utc_datetime_from_timestamp(timestamp);
        assert_eq!(out, expected, "Failed to convert timestamp to DateTime");
    }

    #[test]
    #[should_panic(expected = "Failed to convert timestamp to NaiveDateTime")]
    fn test_must_utc_datetime_from_timestamp_incorrect_timestamp() {
        let timestamp = i64::MAX as u64 + 1;
        let _result = must_utc_datetime_from_timestamp(timestamp);
    }

    #[test]
    fn test_utc_dt_string_from_timestamp() {
        let timestamp = 1633027200;
        let expected = "2021-09-30T18:40:00+00:00";
        let out = utc_dt_string_from_timestamp(timestamp);
        println!("{}", out);
        assert_eq!(out, expected, "Failed to convert timestamp to String");
    }
}

use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::str::FromStr;
use std::time::Duration;

use anyhow::Result;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;
use ipfs_api_backend_hyper::{IpfsApi, IpfsClient, TryFromUri};
use once_cell::sync::Lazy;
use reqwest::Client;
use sqlx::{Column, Row, TypeInfo};
use starknet::core::types::U256;
use starknet_crypto::Felt;
use tokio_util::bytes::Bytes;
use tracing::debug;

use crate::constants::{
    IPFS_CLIENT_PASSWORD, IPFS_CLIENT_URL, IPFS_CLIENT_USERNAME, REQ_MAX_RETRIES,
    SQL_FELT_DELIMITER,
};

pub fn must_utc_datetime_from_timestamp(timestamp: u64) -> DateTime<Utc> {
    let naive_dt = DateTime::from_timestamp(timestamp as i64, 0)
        .expect("Failed to convert timestamp to NaiveDateTime");
    naive_dt.to_utc()
}

pub fn utc_dt_string_from_timestamp(timestamp: u64) -> String {
    must_utc_datetime_from_timestamp(timestamp).to_rfc3339()
}

pub fn felts_to_sql_string(felts: &[Felt]) -> String {
    felts.iter().map(|k| format!("{:#x}", k)).collect::<Vec<String>>().join(SQL_FELT_DELIMITER)
        + SQL_FELT_DELIMITER
}

pub fn felt_to_sql_string(felt: &Felt) -> String {
    format!("{:#x}", felt)
}

pub fn felt_and_u256_to_sql_string(felt: &Felt, u256: &U256) -> String {
    format!("{}:{}", felt_to_sql_string(felt), u256_to_sql_string(u256))
}

pub fn u256_to_sql_string(u256: &U256) -> String {
    format!("{:#064x}", u256)
}

pub fn sql_string_to_u256(sql_string: &str) -> U256 {
    let sql_string = sql_string.strip_prefix("0x").unwrap_or(sql_string);
    U256::from(crypto_bigint::U256::from_be_hex(sql_string))
}

pub fn sql_string_to_felts(sql_string: &str) -> Vec<Felt> {
    sql_string.split(SQL_FELT_DELIMITER).map(|felt| Felt::from_str(felt).unwrap()).collect()
}

/// Sanitizes a JSON string by escaping unescaped double quotes within string values.
pub fn sanitize_json_string(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();
    let mut in_string = false;
    let mut backslash_count = 0;

    while let Some(c) = chars.next() {
        if !in_string {
            if c == '"' {
                in_string = true;
                backslash_count = 0;
                result.push('"');
            } else {
                result.push(c);
            }
        } else if c == '\\' {
            backslash_count += 1;
            result.push('\\');
        } else if c == '"' {
            if backslash_count % 2 == 0 {
                // Unescaped double quote
                let mut temp_chars = chars.clone();
                // Skip whitespace
                while let Some(&next_c) = temp_chars.peek() {
                    if next_c.is_whitespace() {
                        temp_chars.next();
                    } else {
                        break;
                    }
                }
                // Check next non-whitespace character
                if let Some(&next_c) = temp_chars.peek() {
                    if next_c == ':' || next_c == ',' || next_c == '}' {
                        // End of string
                        result.push('"');
                        in_string = false;
                    } else {
                        // Internal unescaped quote, escape it
                        result.push_str("\\\"");
                    }
                } else {
                    // End of input, treat as end of string
                    result.push('"');
                    in_string = false;
                }
            } else {
                // Escaped double quote, part of string
                result.push('"');
            }
            backslash_count = 0;
        } else {
            result.push(c);
            backslash_count = 0;
        }
    }

    result
}

// Global clients
static HTTP_CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .expect("Failed to create HTTP client")
});

static IPFS_CLIENT: Lazy<IpfsClient> = Lazy::new(|| {
    IpfsClient::from_str(IPFS_CLIENT_URL)
        .expect("Failed to create IPFS client")
        .with_credentials(IPFS_CLIENT_USERNAME, IPFS_CLIENT_PASSWORD)
});

const INITIAL_BACKOFF: Duration = Duration::from_millis(100);

/// Fetch content from HTTP URL with retries
pub async fn fetch_content_from_http(url: &str) -> Result<Bytes> {
    let mut retries = 0;
    let mut backoff = INITIAL_BACKOFF;

    loop {
        match HTTP_CLIENT.get(url).send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    return Err(anyhow::anyhow!(
                        "HTTP request failed with status: {}",
                        response.status()
                    ));
                }
                return response.bytes().await.map_err(Into::into);
            }
            Err(e) => {
                if retries >= REQ_MAX_RETRIES {
                    return Err(anyhow::anyhow!("HTTP request failed: {}", e));
                }
                debug!(error = %e, retry = retries, "Request failed, retrying after backoff");
                tokio::time::sleep(backoff).await;
                retries += 1;
                backoff *= 2;
            }
        }
    }
}

/// Fetch content from IPFS with retries
pub async fn fetch_content_from_ipfs(cid: &str) -> Result<Bytes> {
    let mut retries = 0;
    let mut backoff = INITIAL_BACKOFF;

    loop {
        match IPFS_CLIENT.cat(cid).map_ok(|chunk| chunk.to_vec()).try_concat().await {
            Ok(stream) => return Ok(Bytes::from(stream)),
            Err(e) => {
                if retries >= REQ_MAX_RETRIES {
                    return Err(anyhow::anyhow!("IPFS request failed: {}", e));
                }
                debug!(error = %e, retry = retries, "Request failed, retrying after backoff");
                tokio::time::sleep(backoff).await;
                retries += 1;
                backoff *= 2;
            }
        }
    }
}

// Map a SQLite row to a JSON value
pub fn map_row_to_json(row: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    for (i, column) in row.columns().iter().enumerate() {
        let value: serde_json::Value = match column.type_info().name() {
            "TEXT" => row
                .get::<Option<String>, _>(i)
                .map_or(serde_json::Value::Null, serde_json::Value::String),
            "INTEGER" => row
                .get::<Option<i64>, _>(i)
                .map_or(serde_json::Value::Null, |n| serde_json::Value::Number(n.into())),
            "REAL" => row.get::<Option<f64>, _>(i).map_or(serde_json::Value::Null, |f| {
                serde_json::Number::from_f64(f)
                    .map_or(serde_json::Value::Null, serde_json::Value::Number)
            }),
            "BLOB" => row.get::<Option<Vec<u8>>, _>(i).map_or(serde_json::Value::Null, |bytes| {
                serde_json::Value::String(STANDARD.encode(bytes))
            }),
            _ => {
                // Try different types in order
                if let Ok(val) = row.try_get::<i64, _>(i) {
                    serde_json::Value::Number(val.into())
                } else if let Ok(val) = row.try_get::<f64, _>(i) {
                    serde_json::json!(val)
                } else if let Ok(val) = row.try_get::<bool, _>(i) {
                    serde_json::Value::Bool(val)
                } else if let Ok(val) = row.try_get::<String, _>(i) {
                    serde_json::Value::String(val)
                } else {
                    // Handle or fallback to BLOB as base64
                    let val = row.get::<Option<Vec<u8>>, _>(i);
                    val.map_or(serde_json::Value::Null, |bytes| {
                        serde_json::Value::String(STANDARD.encode(bytes))
                    })
                }
            }
        };
        obj.insert(column.name().to_string(), value);
    }
    serde_json::Value::Object(obj)
}

// type used to do calculation on inmemory balances
#[derive(Debug, Clone, Copy)]
pub struct I256 {
    pub value: U256,
    pub is_negative: bool,
}

impl Default for I256 {
    fn default() -> Self {
        Self { value: U256::from(0u8), is_negative: false }
    }
}

impl From<U256> for I256 {
    fn from(value: U256) -> Self {
        Self { value, is_negative: false }
    }
}

impl From<u8> for I256 {
    fn from(value: u8) -> Self {
        Self { value: U256::from(value), is_negative: false }
    }
}

impl Add for I256 {
    type Output = I256;

    fn add(self, other: I256) -> I256 {
        // Special case: if both are negative zero, return positive zero
        if self.value == U256::from(0u8)
            && other.value == U256::from(0u8)
            && self.is_negative
            && other.is_negative
        {
            return I256 { value: U256::from(0u8), is_negative: false };
        }

        if self.is_negative == other.is_negative {
            // Same sign: add the values and keep the sign
            I256 { value: self.value + other.value, is_negative: self.is_negative }
        } else {
            // Different signs: subtract the smaller value from the larger one
            match self.value.cmp(&other.value) {
                Ordering::Greater => {
                    I256 { value: self.value - other.value, is_negative: self.is_negative }
                }
                Ordering::Less => {
                    I256 { value: other.value - self.value, is_negative: other.is_negative }
                }
                // If both values are equal, the result is zero and not negative
                Ordering::Equal => I256 { value: U256::from(0u8), is_negative: false },
            }
        }
    }
}

impl Sub for I256 {
    type Output = I256;

    fn sub(self, other: I256) -> I256 {
        let new_sign = if other.value == U256::from(0u8) { false } else { !other.is_negative };
        let negated_other = I256 { value: other.value, is_negative: new_sign };
        self.add(negated_other)
    }
}

impl AddAssign for I256 {
    fn add_assign(&mut self, other: I256) {
        *self = *self + other;
    }
}

impl SubAssign for I256 {
    fn sub_assign(&mut self, other: I256) {
        *self = *self - other;
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, NaiveDate, NaiveTime, Utc};

    use super::*;

    #[test]
    fn test_sanitize_json_string() {
        let input = r#"{"name":""Rage Shout" DireWolf"}"#;
        let expected = r#"{"name":"\"Rage Shout\" DireWolf"}"#;
        let sanitized = sanitize_json_string(input);
        assert_eq!(sanitized, expected);

        let input_escaped = r#"{"name":"\"Properly Escaped\" Wolf"}"#;
        let expected_escaped = r#"{"name":"\"Properly Escaped\" Wolf"}"#;
        let sanitized_escaped = sanitize_json_string(input_escaped);
        assert_eq!(sanitized_escaped, expected_escaped);
    }

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

    #[test]
    fn test_add_zero_false_and_zero_false() {
        // 0,false + 0,false == 0,false
        let a = I256::default();
        let b = I256::default();
        let result = a + b;
        assert_eq!(result.value, U256::from(0u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_add_zero_true_and_zero_false() {
        // 0,true + 0,false == 0,false
        let a = I256 { value: U256::from(0u8), is_negative: true };
        let b = I256::default();
        let result = a + b;
        assert_eq!(result.value, U256::from(0u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_sub_zero_false_and_zero_false() {
        // 0,false - 0,false == 0,false
        let a = I256::default();
        let b = I256::default();
        let result = a - b;
        assert_eq!(result.value, U256::from(0u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_sub_zero_true_and_zero_false() {
        // 0,true - 0,false == 0,false
        let a = I256 { value: U256::from(0u8), is_negative: true };
        let b = I256::default();
        let result = a - b;
        assert_eq!(result.value, U256::from(0u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_add_positive_and_negative_equal_values() {
        // 5,false + 5,true == 0,false
        let a = I256::from(U256::from(5u8));
        let b = I256 { value: U256::from(5u8), is_negative: true };
        let result = a + b;
        assert_eq!(result.value, U256::from(0u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_sub_positive_and_negative() {
        // 10,false - 5,true == 15,false
        let a = I256::from(U256::from(10u8));
        let b = I256 { value: U256::from(5u8), is_negative: true };
        let result = a - b;
        assert_eq!(result.value, U256::from(15u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_sub_larger_from_smaller() {
        // 5,false - 10,true == 15,true
        let a = I256::from(U256::from(5u8));
        let b = I256 { value: U256::from(10u8), is_negative: true };
        let result = a - b;
        assert_eq!(result.value, U256::from(15u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_add_mixed_signs() {
        // 15,false + 10,true == 5,false
        let a = I256::from(U256::from(15u8));
        let b = I256 { value: U256::from(10u8), is_negative: true };
        let result = a + b;
        assert_eq!(result.value, U256::from(5u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_sub_mixed_signs() {
        // 5,false - 10,true == 15,false
        let a = I256::from(U256::from(5u8));
        let b = I256 { value: U256::from(10u8), is_negative: true };
        let result = a - b;
        assert_eq!(result.value, U256::from(15u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_add_negative_and_negative() {
        // -5,true + -3,true == -8,true
        let a = I256 { value: U256::from(5u8), is_negative: true };
        let b = I256 { value: U256::from(3u8), is_negative: true };
        let result = a + b;
        assert_eq!(result.value, U256::from(8u8));
        assert!(result.is_negative);
    }

    #[test]
    fn test_sub_negative_and_negative() {
        // -5,true - -3,true == -2,true
        let a = I256 { value: U256::from(5u8), is_negative: true };
        let b = I256 { value: U256::from(3u8), is_negative: true };
        let result = a - b;
        assert_eq!(result.value, U256::from(2u8));
        assert!(result.is_negative);
    }

    #[test]
    fn test_subtraction_resulting_zero() {
        // 5,false - 5,false == 0,false
        let a = I256::from(U256::from(5u8));
        let b = I256::from(U256::from(5u8));
        let result = a - b;
        assert_eq!(result.value, U256::from(0u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_subtraction_resulting_zero_negative() {
        // 5,true - 5,true == 0,false
        let a = I256 { value: U256::from(5u8), is_negative: true };
        let b = I256 { value: U256::from(5u8), is_negative: true };
        let result = a - b;
        assert_eq!(result.value, U256::from(0u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_add_negative_and_positive_result_positive() {
        // -10,true + 15,false == 5,false
        let a = I256 { value: U256::from(10u8), is_negative: true };
        let b = I256::from(U256::from(15u8));
        let result = a + b;
        assert_eq!(result.value, U256::from(5u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_add_negative_and_positive_result_negative() {
        // -15,true + 5,false == -10,true
        let a = I256 { value: U256::from(15u8), is_negative: true };
        let b = I256::from(U256::from(5u8));
        let result = a + b;
        assert_eq!(result.value, U256::from(10u8));
        assert!(result.is_negative);
    }

    #[test]
    fn test_add_zero_true_and_fifteen_true() {
        // 0,true + 15,true == 15,true
        let a = I256 { value: U256::from(0u8), is_negative: true };
        let b = I256 { value: U256::from(15u8), is_negative: true };
        let result = a + b;
        assert_eq!(result.value, U256::from(15u8));
        assert!(result.is_negative);
    }

    #[test]
    fn test_sub_zero_true_and_fifteen_true() {
        // 0,true - 15,true == 15,false
        let a = I256 { value: U256::from(0u8), is_negative: true };
        let b = I256 { value: U256::from(15u8), is_negative: true };
        let result = a - b;
        assert_eq!(result.value, U256::from(15u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_add_fifteen_true_and_zero_true() {
        // 15,true + 0,true == 15,true
        let a = I256 { value: U256::from(15u8), is_negative: true };
        let b = I256 { value: U256::from(0u8), is_negative: true };
        let result = a + b;
        assert_eq!(result.value, U256::from(15u8));
        assert!(result.is_negative);
    }

    #[test]
    fn test_sub_fifteen_true_and_zero_true() {
        // 15,true - 0,true == 15,true
        let a = I256 { value: U256::from(15u8), is_negative: true };
        let b = I256 { value: U256::from(0u8), is_negative: true };
        let result = a - b;
        assert_eq!(result.value, U256::from(15u8));
        assert!(result.is_negative);
    }

    #[test]
    fn test_negative_zero() {
        // 0,true + 0,true == 0,false
        let a = I256 { value: U256::from(0u8), is_negative: true };
        let b = I256 { value: U256::from(0u8), is_negative: true };
        let result = a + b;
        assert_eq!(result.value, U256::from(0u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_sub_positive_and_negative_zero() {
        // 15,false - 0,true == 15,false
        let a = I256::from(U256::from(15u8));
        let b = I256 { value: U256::from(0u8), is_negative: true };
        let result = a - b;
        assert_eq!(result.value, U256::from(15u8));
        assert!(!result.is_negative);
    }

    #[test]
    fn test_add_positive_and_negative_zero() {
        // 15,false + 0,true == 15,false
        let a = I256::from(U256::from(15u8));
        let b = I256 { value: U256::from(0u8), is_negative: true };
        let result = a + b;
        assert_eq!(result.value, U256::from(15u8));
        assert!(!result.is_negative);
    }
}

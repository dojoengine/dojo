use chrono::{DateTime, NaiveDateTime, Utc};

pub fn utc_datetime_from_timestamp(timestamp: u64) -> DateTime<Utc> {
    let naive_dt = NaiveDateTime::from_timestamp_opt(timestamp as i64, 0)
        .expect("Failed to convert timestamp to NaiveDateTime");
    DateTime::<Utc>::from_naive_utc_and_offset(naive_dt, Utc)
}

pub fn utc_dt_string_from_timestamp(timestamp: u64) -> String {
    utc_datetime_from_timestamp(timestamp).to_rfc3339()
}

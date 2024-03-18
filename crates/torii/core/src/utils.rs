use chrono::{DateTime, NaiveDateTime, Utc};

pub fn timestamp_to_utc_datetime(timestamp: u64) -> DateTime<Utc> {
    let timestamp_as_dt = NaiveDateTime::from_timestamp_opt(timestamp as i64, 0).expect("err");
    DateTime::<Utc>::from_naive_utc_and_offset(timestamp_as_dt, Utc)
}

pub fn timestamp_to_str_utc_date(timestamp: u64) -> String {
    let dt = timestamp_to_utc_datetime(timestamp);
    dt.to_rfc3339()
}

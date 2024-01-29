use std::time::SystemTime;

pub(super) fn get_current_timestamp() -> std::time::Duration {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("should get current UNIX timestamp")
}

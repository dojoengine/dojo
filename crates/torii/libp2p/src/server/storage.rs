use sqlx::{Pool, Sqlite};

pub(crate) struct RelayStorage {
    pub(crate) pool: Pool<Sqlite>,
}

impl RelayStorage {
    pub(crate) fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }

    pub(crate) async fn store_message(
        &self,
        message: proto::relay::Message,
    ) -> Result<(), sqlx::Error> {
        let timestamp = message.timestamp.map_or(sqlx::types::chrono::Utc::now(), |ts| {
            sqlx::types::chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
                .expect("Invalid timestamp")
        });

        sqlx::query(
            r#"
            INSERT INTO messages (id, source_id, topic, data, created_at)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(message.message_id)
        .bind(message.source_id)
        .bind(message.topic)
        .bind(message.data)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

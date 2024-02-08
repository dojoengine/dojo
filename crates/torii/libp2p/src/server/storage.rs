use sqlx::{Pool, Row, Sqlite};
use torii_grpc::proto;

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

    pub(crate) async fn get_messages(
        &self,
        topic: String,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<proto::relay::Message>, sqlx::Error> {
        let messages = sqlx::query(
            r#"
            SELECT id, source_id, topic, data, created_at
            FROM messages
            WHERE topic = ?
            ORDER BY created_at DESC
            LIMIT ?
            OFFSET ?
            "#,
        )
        .bind(topic)
        .bind(limit)
        .bind(offset)
        // Convert the SQL rows into protobuf messages.
        .map(|row: sqlx::sqlite::SqliteRow| {
            let timestamp = row.get::<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>, _>(4);
            let timestamp = prost_types::Timestamp {
                seconds: timestamp.timestamp(),
                nanos: timestamp.timestamp_subsec_nanos() as i32,
            };

            proto::relay::Message {
                message_id: row.get(0),
                source_id: row.get(1),
                topic: row.get(2),
                data: row.get(3),
                timestamp: Some(timestamp),
            }
        })
        .fetch_all(&self.pool)
        .await?;

        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use async_graphql::dynamic::Schema;
    use serde_json::Value;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};

    use crate::schema::build_schema;
    use crate::tests::{run_graphql_query, Connection, Event};

    async fn events_query(schema: &Schema, args: &str) -> Value {
        let query = format!(
            r#"
          {{
            events {} {{
              totalCount
              edges {{
                cursor
                node {{
                  id
                  keys
                  data
                  transactionHash
                }}
              }}
              pageInfo {{
                hasPreviousPage
                hasNextPage
                startCursor
                endCursor
              }}
            }}
          }}
        "#,
            args
        );

        let result = run_graphql_query(schema, &query).await;
        result.get("events").ok_or("events not found").unwrap().clone()
    }

    #[sqlx::test(migrations = "../migrations", fixtures("./fixtures/events.sql"))]
    async fn test_events_query(
        options: SqlitePoolOptions,
        mut connect_options: SqliteConnectOptions,
    ) -> Result<()> {
        // enable regex
        connect_options = connect_options.with_regexp();

        let pool = options.connect_with(connect_options).await?;
        let schema = build_schema(&pool).await?;

        let result = events_query(&schema, "(keys: [\"0x1\"])").await;
        let connection: Connection<Event> = serde_json::from_value(result.clone())?;
        let event = connection.edges.first().unwrap();
        assert_eq!(connection.total_count, 1);
        assert_eq!(event.node.id, "0x1");

        let result = events_query(&schema, "(keys: [\"0x2\", \"*\", \"0x1\"])").await;
        let connection: Connection<Event> = serde_json::from_value(result.clone())?;
        let event = connection.edges.first().unwrap();
        assert_eq!(connection.total_count, 1);
        assert_eq!(event.node.id, "0x2");

        let result = events_query(&schema, "(keys: [\"*\", \"0x1\"])").await;
        let connection: Connection<Event> = serde_json::from_value(result.clone())?;
        let event = connection.edges.first().unwrap();
        assert_eq!(connection.total_count, 1);
        assert_eq!(event.node.id, "0x3");

        Ok(())
    }
}

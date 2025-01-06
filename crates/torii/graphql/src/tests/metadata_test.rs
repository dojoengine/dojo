#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use dojo_world::config::{ProfileConfig, WorldMetadata};
    use sqlx::SqlitePool;
    use starknet::core::types::Felt;
    use starknet::providers::jsonrpc::HttpTransport;
    use starknet::providers::JsonRpcClient;
    use tokio::sync::broadcast;
    use torii_sqlite::executor::Executor;
    use torii_sqlite::cache::ModelCache;
    use torii_sqlite::Sql;
    use torii_sqlite::types::{Contract, ContractType};
    use url::Url;

    use crate::schema::build_schema;
    use crate::tests::{run_graphql_query, Connection, Content, Metadata as SqlMetadata, Social};

    const RESOURCE: Felt = Felt::ZERO;
    const URI: &str = "ipfs://QmcDVFdDph5N2AoW7L2vruyhy6A3wiU8Mh5hEyfVY68ynh";
    const BLOCK_TIMESTAMP: u64 = 1710754478;
    const QUERY: &str = r#"
      {
        metadatas {
          totalCount
          edges {
            cursor
            node {
              uri
              worldAddress
              coverImg
              iconImg
              content {
                name
                description
                coverUri
                iconUri
                website
                socials {
                  name
                  url
                }
              }
            }
          }
          pageInfo {
            hasPreviousPage
            hasNextPage
            startCursor
            endCursor
          }
        }
      }
    "#;

    #[sqlx::test(migrations = "../migrations")]
    async fn test_metadata(pool: SqlitePool) {
        let (shutdown_tx, _) = broadcast::channel(1);
        let url: Url = "https://www.example.com".parse().unwrap();
        let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(url)));
        let (mut executor, sender) =
            Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100)
                .await
                .unwrap();
        tokio::spawn(async move {
            executor.run().await.unwrap();
        });
        let model_cache = Arc::new(ModelCache::new(pool.clone()));
        let mut db = Sql::new(
            pool.clone(),
            sender,
            &[Contract { address: Felt::ZERO, r#type: ContractType::WORLD }],
            model_cache,
        )
        .await
        .unwrap();
        let schema = build_schema(&pool).await.unwrap();

        let cover_img = "QWxsIHlvdXIgYmFzZSBiZWxvbmcgdG8gdXM=";
        let profile_config: ProfileConfig = toml::from_str(
            r#"
  [world]
  name = "example"
  description = "example world"
  seed = "example"
  cover_uri = "file://example_cover.png"
  website = "https://dojoengine.org"
  socials.x = "https://x.com/dojostarknet"

  [namespace]
  default = "example"
          "#,
        )
        .unwrap();
        // TODO: we may want to store here the namespace and the seed. Check the
        // implementation to actually add those to the metadata table.
        let world_metadata: WorldMetadata = profile_config.world.into();
        db.set_metadata(&RESOURCE, URI, BLOCK_TIMESTAMP).unwrap();
        db.update_metadata(&RESOURCE, URI, &world_metadata, &None, &Some(cover_img.to_string()))
            .unwrap();
        db.execute().await.unwrap();

        let result = run_graphql_query(&schema, QUERY).await;
        let value = result.get("metadatas").ok_or("metadatas not found").unwrap().clone();
        let connection: Connection<SqlMetadata> = serde_json::from_value(value).unwrap();
        let edge = connection.edges.first().unwrap();
        assert_eq!(edge.node.world_address, "0x0");
        assert_eq!(connection.edges.len(), 1);
        assert_eq!(edge.node.cover_img, cover_img);
        assert_eq!(
            edge.node.content,
            Content {
                name: Some("example".into()),
                description: Some("example world".into()),
                website: Some("https://dojoengine.org/".into()),
                icon_uri: None,
                cover_uri: Some("file://example_cover.png".into()),
                socials: vec![Social {
                    name: "x".into(),
                    url: "https://x.com/dojostarknet".into()
                }]
            }
        )
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn test_empty_content(pool: SqlitePool) {
        let (shutdown_tx, _) = broadcast::channel(1);
        let url: Url = "https://www.example.com".parse().unwrap();
        let provider = Arc::new(JsonRpcClient::new(HttpTransport::new(url)));
        let (mut executor, sender) =
            Executor::new(pool.clone(), shutdown_tx.clone(), Arc::clone(&provider), 100)
                .await
                .unwrap();
        tokio::spawn(async move {
            executor.run().await.unwrap();
        });

        let model_cache = Arc::new(ModelCache::new(pool.clone()));
        let mut db = Sql::new(
            pool.clone(),
            sender,
            &[Contract { address: Felt::ZERO, r#type: ContractType::WORLD }],
            model_cache,
        )
        .await
        .unwrap();
        let schema = build_schema(&pool).await.unwrap();

        db.set_metadata(&RESOURCE, URI, BLOCK_TIMESTAMP).unwrap();
        db.execute().await.unwrap();

        let result = run_graphql_query(&schema, QUERY).await;
        let value = result.get("metadatas").ok_or("metadatas not found").unwrap().clone();
        let connection: Connection<SqlMetadata> = serde_json::from_value(value).unwrap();
        let edge = connection.edges.first().unwrap();
        assert_eq!(
            edge.node.content,
            Content {
                name: None,
                description: None,
                website: None,
                icon_uri: None,
                cover_uri: None,
                socials: vec![]
            }
        );
    }
}

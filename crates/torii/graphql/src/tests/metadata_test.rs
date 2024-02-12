#[cfg(test)]
mod tests {
    use dojo_world::metadata::Metadata as DojoMetadata;
    use sqlx::SqlitePool;
    use starknet_crypto::FieldElement;
    use torii_core::sql::Sql;

    use crate::schema::build_schema;
    use crate::tests::{run_graphql_query, Connection, Content, Metadata as SqlMetadata, Social};

    const RESOURCE: FieldElement = FieldElement::ZERO;
    const URI: &str = "ipfs://QmcDVFdDph5N2AoW7L2vruyhy6A3wiU8Mh5hEyfVY68ynh";
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
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        let schema = build_schema(&pool).await.unwrap();

        let cover_img = "QWxsIHlvdXIgYmFzZSBiZWxvbmcgdG8gdXM=";
        let dojo_metadata: DojoMetadata = toml::from_str(
            r#"
  [world]
  name = "example"
  description = "example world"
  cover_uri = "file://example_cover.png"
  website = "https://dojoengine.org"
  socials.x = "https://x.com/dojostarknet"
          "#,
        )
        .unwrap();
        let world_metadata = dojo_metadata.world.unwrap();
        db.update_metadata(&RESOURCE, URI, &world_metadata, &None, &Some(cover_img.to_string()))
            .await
            .unwrap();

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
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        let schema = build_schema(&pool).await.unwrap();

        db.set_metadata(&RESOURCE, URI);
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

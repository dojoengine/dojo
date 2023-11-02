#[cfg(test)]
mod tests {
    use dojo_world::metadata::Metadata as DojoMetadata;
    use sqlx::SqlitePool;
    use starknet_crypto::FieldElement;
    use torii_core::sql::Sql;

    use crate::schema::build_schema;
    use crate::tests::{run_graphql_query, Connection, Metadata as SqlMetadata, Social};

    #[sqlx::test(migrations = "../migrations")]
    async fn test_metadata(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        let schema = build_schema(&pool).await.unwrap();

        let resource = FieldElement::ZERO;
        let uri = "ipfs://QmcDVFdDph5N2AoW7L2vruyhy6A3wiU8Mh5hEyfVY68ynh";
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
        db.update_metadata(&resource, uri, &world_metadata, &None, &Some(cover_img.to_string()))
            .await
            .unwrap();

        db.execute().await.unwrap();

        let query = r#"
          {
            metadatas {
              total_count
              edges {
                cursor
                node {
                  uri
                  cover_img
                  icon_img
                  content {
                    name
                    description
                    cover_uri
                    icon_uri
                    socials {
                      name
                      url
                    }
                  }
                }
              }
            }
          }
        "#;

        let result = run_graphql_query(&schema, query).await;
        let value = result.get("metadatas").ok_or("metadatas not found").unwrap().clone();
        let connection: Connection<SqlMetadata> = serde_json::from_value(value).unwrap();
        let edge = connection.edges.first().unwrap();
        assert_eq!(connection.edges.len(), 1);
        assert_eq!(edge.node.content.name, Some("example".into()));
        assert_eq!(edge.node.content.description, Some("example world".into()));
        assert_eq!(edge.node.content.icon_uri, None);
        assert_eq!(edge.node.content.cover_uri, Some("file://example_cover.png".into()));
        assert_eq!(
            edge.node.content.socials.as_ref().unwrap(),
            &vec![Social { name: "x".into(), url: "https://x.com/dojostarknet".into() }]
        );
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use async_graphql::dynamic::Schema;
    use serde_json::Value;
    use starknet::core::types::Felt;
    use starknet_crypto::poseidon_hash_many;
    use tempfile::NamedTempFile;

    use crate::schema::build_schema;
    use crate::tests::{
        run_graphql_query, spinup_types_test, Connection, Entity, Record, RecordSibling, Subrecord,
    };

    async fn entities_query(schema: &Schema, arg: &str) -> Value {
        let query = format!(
            r#"
          {{
            entities {} {{
              totalCount
              edges {{
                cursor
                node {{
                  keys
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
            arg,
        );

        let result = run_graphql_query(schema, &query).await;
        result.get("entities").ok_or("entities not found").unwrap().clone()
    }

    async fn entity_model_query(schema: &Schema, id: &Felt) -> Value {
        let query = format!(
            r#"
          {{
            entity (id: "{:#x}") {{
              keys
              models {{
                ... on types_test_Record {{
                  __typename
                  depth
                  record_id
                  type_u8
                  type_u16
                  type_u32
                  type_u64
                  type_u128
                  type_u256
                  type_bool
                  type_felt
                  type_class_hash
                  type_contract_address
                  random_u8
                  random_u128
                }}
                ... on types_test_RecordSibling {{
                  __typename
                  record_id
                  random_u8
                }}
                ... on types_test_Subrecord {{
                  __typename
                  record_id
                  subrecord_id
                  type_u8
                  random_u8
                }}
              }}
            }}
          }}
        "#,
            id
        );

        let result = run_graphql_query(schema, &query).await;
        result.get("entity").ok_or("entity not found").unwrap().clone()
    }

    // End to end test spins up a test sequencer and deploys types-test project, this takes a while
    // to run so combine all related tests into one
    #[tokio::test(flavor = "multi_thread")]
    async fn entities_test() -> Result<()> {
        let tempfile = NamedTempFile::new().unwrap();
        let path = tempfile.path().to_string_lossy();
        let pool = spinup_types_test(&path).await?;
        let schema = build_schema(&pool).await.unwrap();

        // default without params
        let entities = entities_query(&schema, "").await;
        let connection: Connection<Entity> = serde_json::from_value(entities).unwrap();
        assert_eq!(connection.edges.len(), 10);
        assert_eq!(connection.total_count, 20);

        // first key param - returns all entities with `0x0` as first key
        let entities = entities_query(&schema, "(keys: [\"0x0\"])").await;
        let connection: Connection<Entity> = serde_json::from_value(entities).unwrap();
        let first_entity = connection.edges.first().unwrap();
        let last_entity = connection.edges.last().unwrap();
        assert_eq!(connection.edges.len(), 2);
        assert_eq!(connection.total_count, 2);
        // due to parallelization order is nondeterministic
        assert!(
            first_entity.node.keys.clone().unwrap() == vec!["0x0", "0x1"]
                || first_entity.node.keys.clone().unwrap() == vec!["0x0"]
        );
        assert!(
            last_entity.node.keys.clone().unwrap() == vec!["0x0", "0x1"]
                || last_entity.node.keys.clone().unwrap() == vec!["0x0"]
        );

        // double key param - returns all entities with `0x0` as first key and `0x1` as second key
        let entities = entities_query(&schema, "(keys: [\"0x0\", \"0x1\"])").await;
        let connection: Connection<Entity> = serde_json::from_value(entities).unwrap();
        let first_entity = connection.edges.first().unwrap();
        assert_eq!(connection.edges.len(), 1);
        assert_eq!(connection.total_count, 1);
        assert_eq!(first_entity.node.keys.clone().unwrap(), vec!["0x0", "0x1"]);

        // pagination testing
        let entities = entities_query(&schema, "(first: 20)").await;
        let all_entities_connection: Connection<Entity> = serde_json::from_value(entities).unwrap();
        let one = all_entities_connection.edges.first().unwrap();
        let two = all_entities_connection.edges.get(1).unwrap();
        let three = all_entities_connection.edges.get(2).unwrap();
        let four = all_entities_connection.edges.get(3).unwrap();
        let five = all_entities_connection.edges.get(4).unwrap();
        let six = all_entities_connection.edges.get(5).unwrap();
        let seven = all_entities_connection.edges.get(6).unwrap();

        // cursor based forward pagination
        let entities =
            entities_query(&schema, &format!("(first: 2, after: \"{}\")", two.cursor)).await;
        let connection: Connection<Entity> = serde_json::from_value(entities).unwrap();
        assert_eq!(connection.edges.len(), 2);
        assert_eq!(connection.edges.first().unwrap(), three);
        assert_eq!(connection.edges.last().unwrap(), four);

        assert!(connection.page_info.has_previous_page);
        assert!(connection.page_info.has_next_page);
        assert_eq!(connection.page_info.start_cursor.unwrap(), three.cursor);
        assert_eq!(connection.page_info.end_cursor.unwrap(), four.cursor);

        let entities =
            entities_query(&schema, &format!("(first: 3, after: \"{}\")", three.cursor)).await;
        let connection: Connection<Entity> = serde_json::from_value(entities).unwrap();
        assert_eq!(connection.edges.len(), 3);
        assert_eq!(connection.edges.first().unwrap(), four);
        assert_eq!(connection.edges.last().unwrap(), six);

        assert!(connection.page_info.has_previous_page);
        assert!(connection.page_info.has_next_page);
        assert_eq!(connection.page_info.start_cursor.unwrap(), four.cursor);
        assert_eq!(connection.page_info.end_cursor.unwrap(), six.cursor);

        // cursor based backward pagination
        let entities =
            entities_query(&schema, &format!("(last: 2, before: \"{}\")", seven.cursor)).await;
        let connection: Connection<Entity> = serde_json::from_value(entities).unwrap();
        assert_eq!(connection.edges.len(), 2);
        assert_eq!(connection.edges.first().unwrap(), six);
        assert_eq!(connection.edges.last().unwrap(), five);

        assert!(connection.page_info.has_previous_page);
        assert!(connection.page_info.has_next_page);
        assert_eq!(connection.page_info.start_cursor.unwrap(), six.cursor);
        assert_eq!(connection.page_info.end_cursor.unwrap(), five.cursor);

        let entities =
            entities_query(&schema, &format!("(last: 3, before: \"{}\")", six.cursor)).await;
        let connection: Connection<Entity> = serde_json::from_value(entities).unwrap();
        assert_eq!(connection.edges.len(), 3);
        assert_eq!(connection.edges.first().unwrap(), five);
        assert_eq!(connection.edges.last().unwrap(), three);

        assert!(connection.page_info.has_previous_page);
        assert!(connection.page_info.has_next_page);
        assert_eq!(connection.page_info.start_cursor.unwrap(), five.cursor);
        assert_eq!(connection.page_info.end_cursor.unwrap(), three.cursor);

        let empty_entities = entities_query(
            &schema,
            &format!(
                "(first: 1, after: \"{}\")",
                all_entities_connection.edges.last().unwrap().cursor
            ),
        )
        .await;
        let connection: Connection<Entity> = serde_json::from_value(empty_entities).unwrap();
        assert_eq!(connection.edges.len(), 0);

        assert!(!connection.page_info.has_previous_page);
        assert!(!connection.page_info.has_next_page);
        assert_eq!(connection.page_info.start_cursor, None);
        assert_eq!(connection.page_info.end_cursor, None);

        // offset/limit based pagination
        let entities = entities_query(&schema, "(limit: 2)").await;
        let connection: Connection<Entity> = serde_json::from_value(entities).unwrap();
        assert_eq!(connection.edges.len(), 2);
        assert_eq!(connection.edges.first().unwrap(), one);
        assert_eq!(connection.edges.last().unwrap(), two);

        assert!(!connection.page_info.has_previous_page);
        assert!(connection.page_info.has_next_page);
        assert_eq!(connection.page_info.start_cursor, None);
        assert_eq!(connection.page_info.end_cursor, None);

        let entities = entities_query(&schema, "(limit: 3, offset: 2)").await;
        let connection: Connection<Entity> = serde_json::from_value(entities).unwrap();
        assert_eq!(connection.edges.len(), 3);
        assert_eq!(connection.edges.first().unwrap(), three);
        assert_eq!(connection.edges.last().unwrap(), five);

        assert!(connection.page_info.has_previous_page);
        assert!(connection.page_info.has_next_page);
        assert_eq!(connection.page_info.start_cursor, None);
        assert_eq!(connection.page_info.end_cursor, None);

        let empty_entities = entities_query(&schema, "(limit: 1, offset: 20)").await;
        let connection: Connection<Entity> = serde_json::from_value(empty_entities).unwrap();
        assert_eq!(connection.edges.len(), 0);

        assert!(!connection.page_info.has_previous_page);
        assert!(!connection.page_info.has_next_page);
        assert_eq!(connection.page_info.start_cursor, None);
        assert_eq!(connection.page_info.end_cursor, None);

        // entity model union
        let id = poseidon_hash_many(&[Felt::ZERO]);
        let entity = entity_model_query(&schema, &id).await;
        let models = entity.get("models").ok_or("no models found").unwrap();

        // models should contain record & recordsibling
        let record: Record = serde_json::from_value(models[0].clone()).unwrap();
        assert_eq!(&record.__typename, "types_test_Record");
        assert_eq!(record.record_id, 0);

        let record_sibling: RecordSibling = serde_json::from_value(models[1].clone()).unwrap();
        assert_eq!(&record_sibling.__typename, "types_test_RecordSibling");
        assert_eq!(record_sibling.record_id, 0);

        let id = poseidon_hash_many(&[Felt::ZERO, Felt::ONE]);
        let entity = entity_model_query(&schema, &id).await;
        let models = entity.get("models").ok_or("no models found").unwrap();
        let subrecord: Subrecord = serde_json::from_value(models[0].clone()).unwrap();
        assert_eq!(&subrecord.__typename, "types_test_Subrecord");
        assert_eq!(subrecord.subrecord_id, 1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use anyhow::Result;
    use async_graphql::dynamic::Schema;
    use serde_json::Value;
    use starknet_crypto::FieldElement;

    use crate::schema::build_schema;
    use crate::tests::{run_graphql_query, spinup_types_test, Connection, Record};

    async fn records_model_query(schema: &Schema, arg: &str) -> Value {
        let query = format!(
            r#"
          {{
             recordModels {} {{
              totalCount
              edges {{
                cursor
                node {{
                    __typename
                    record_id
                    depth
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
                    type_deeply_nested {{
                        __typename
                        depth
                        type_number
                        type_string
                        type_nested_more {{
                            __typename
                            depth
                            type_number
                            type_string
                            type_nested_most {{
                                __typename
                                depth
                                type_number
                                type_string
                            }}
                        }}
                    }}
                    type_nested_one {{
                        __typename
                        depth
                        type_number
                        type_string
                    }}
                    type_nested_two {{
                        __typename
                        depth
                        type_number
                        type_string
                    }}
                    entity {{
                        keys
                    }}
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
        result.get("recordModels").ok_or("recordModels not found").unwrap().clone()
    }

    // End to end test spins up a test sequencer and deploys types-test project, this takes a while
    // to run so combine all related tests into one
    #[tokio::test(flavor = "multi_thread")]
    async fn models_test() -> Result<()> {
        let pool = spinup_types_test().await?;
        let schema = build_schema(&pool).await.unwrap();

        // default params, test entity relationship, test nested types
        let records = records_model_query(&schema, "").await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let record = connection.edges.last().unwrap();
        let entity = record.node.entity.as_ref().unwrap();
        let deeply_nested = record.node.type_deeply_nested.as_ref().unwrap();
        let deeply_nested_more = &deeply_nested.type_nested_more;
        let deeply_nested_most = &deeply_nested_more.type_nested_most;
        let nested_one = record.node.type_nested_one.as_ref().unwrap();
        let nested_two = record.node.type_nested_two.as_ref().unwrap();
        assert_eq!(connection.total_count, 10);
        assert_eq!(connection.edges.len(), 10);
        assert_eq!(&record.node.__typename, "Record");
        assert_eq!(entity.keys.clone().unwrap(), vec!["0x0"]);
        assert_eq!(record.node.depth, "Zero");
        assert_eq!(deeply_nested.depth, "One");
        assert_eq!(deeply_nested_more.depth, "Two");
        assert_eq!(deeply_nested_most.depth, "Three");
        assert_eq!(nested_one.type_number, 1);
        assert_eq!(nested_two.type_number, 2);

        // *** WHERE FILTER TESTING ***

        // where filter EQ on u8
        let records = records_model_query(&schema, "(where: { type_u8: 0 })").await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record = connection.edges.first().unwrap();
        assert_eq!(connection.total_count, 1);
        assert_eq!(first_record.node.record_id, 0);

        // where filter GTE on u16
        let records = records_model_query(&schema, "(where: { type_u16GTE: 5 })").await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        assert_eq!(connection.total_count, 5);

        // where filter LTE on u32
        let records = records_model_query(&schema, "(where: { type_u32LTE: 4 })").await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        assert_eq!(connection.total_count, 5);

        // where filter LT and GT
        let records =
            records_model_query(&schema, "(where: { type_u32GT: 2, type_u64LT: 4 })").await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record = connection.edges.first().unwrap();
        assert_eq!(first_record.node.type_u64, 3);

        // NOTE: Server side is gonna parse "0x5" and "5" to hexadecimal format
        let felt_str_0x5 = "0x5";
        let felt_int_5 = "5";

        // where filter EQ on class_hash and contract_address
        let records = records_model_query(
            &schema,
            &format!(
                "(where: {{ type_class_hash: \"{}\", type_contract_address: \"{}\" }})",
                felt_str_0x5, felt_int_5
            ),
        )
        .await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record = connection.edges.first().unwrap();
        assert_eq!(first_record.node.type_class_hash, "0x5");

        // where filter GTE on u128 (string)
        let records = records_model_query(
            &schema,
            &format!("(where: {{ type_u128GTE: \"{}\" }})", felt_str_0x5),
        )
        .await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record = connection.edges.first().unwrap();
        let last_record = connection.edges.last().unwrap();
        assert_eq!(connection.total_count, 5);
        assert_eq!(first_record.node.type_u128, "0x9");
        assert_eq!(last_record.node.type_u128, "0x5");

        // where filter LT on u256 (string)
        let records = records_model_query(
            &schema,
            &format!("(where: {{ type_u256LT: \"{}\" }})", felt_int_5),
        )
        .await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record = connection.edges.first().unwrap();
        let last_record = connection.edges.last().unwrap();
        assert_eq!(connection.total_count, 5);
        assert_eq!(first_record.node.type_u256, "0x4");
        assert_eq!(last_record.node.type_u256, "0x0");

        // where filter on true bool
        let records = records_model_query(&schema, "(where: { type_bool: true })").await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record = connection.edges.first().unwrap();
        assert_eq!(connection.total_count, 5);
        assert!(first_record.node.type_bool, "should be true");

        // where filter on false bool
        let records = records_model_query(&schema, "(where: { type_bool: false })").await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record = connection.edges.first().unwrap();
        assert_eq!(connection.total_count, 5);
        assert!(!first_record.node.type_bool, "should be false");

        // *** ORDER TESTING ***

        // order on random u8 DESC (number)
        let records =
            records_model_query(&schema, "(order: { field: RANDOM_U8, direction: DESC })").await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record = connection.edges.first().unwrap();
        let last_record = connection.edges.last().unwrap();
        assert_eq!(connection.total_count, 10);
        assert!(first_record.node.random_u8 >= last_record.node.random_u8);

        // order on random u128 ASC (string)
        let records =
            records_model_query(&schema, "(order: { field: RANDOM_U128, direction: ASC })").await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record_felt =
            FieldElement::from_str(&connection.edges.first().unwrap().node.random_u128).unwrap();
        let last_record_felt =
            FieldElement::from_str(&connection.edges.last().unwrap().node.random_u128).unwrap();
        assert_eq!(connection.total_count, 10);
        assert!(first_record_felt <= last_record_felt);

        // *** ORDER + WHERE FILTER TESTING ***

        // order + where filter on felt DESC
        let records = records_model_query(
            &schema,
            &format!(
                "(where: {{ type_feltGTE: \"{}\" }}, order: {{ field: TYPE_FELT, direction: DESC \
                 }})",
                felt_str_0x5
            ),
        )
        .await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record = connection.edges.first().unwrap();
        let last_record = connection.edges.last().unwrap();
        assert_eq!(connection.total_count, 5);
        assert!(first_record.node.type_felt > last_record.node.type_felt);

        // *** WHERE FILTER + PAGINATION TESTING ***

        let records = records_model_query(&schema, "(where: { type_u8GTE: 5 })").await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let one = connection.edges.first().unwrap();
        let two = connection.edges.get(1).unwrap();
        let three = connection.edges.get(2).unwrap();
        let four = connection.edges.get(3).unwrap();
        let five = connection.edges.get(4).unwrap();

        // cursor based pagination
        let records = records_model_query(&schema, "(where: { type_u8GTE: 5 }, first: 2)").await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record = connection.edges.first().unwrap();
        let last_record = connection.edges.last().unwrap();
        assert_eq!(connection.total_count, 5);
        assert_eq!(connection.edges.len(), 2);
        assert_eq!(first_record, one);
        assert_eq!(last_record, two);

        let records = records_model_query(
            &schema,
            &format!("(where: {{ type_u8GTE: 5 }}, first: 3, after: \"{}\")", last_record.cursor),
        )
        .await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record = connection.edges.first().unwrap();
        let second_record = connection.edges.last().unwrap();
        assert_eq!(connection.total_count, 5);
        assert_eq!(connection.edges.len(), 3);
        assert_eq!(first_record, three);
        assert_eq!(second_record, five);

        // offset/limit base pagination
        let records =
            records_model_query(&schema, "(where: { type_u8GTE: 5 }, limit: 2, offset: 2)").await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record = connection.edges.first().unwrap();
        let last_record = connection.edges.last().unwrap();
        assert_eq!(connection.total_count, 5);
        assert_eq!(connection.edges.len(), 2);
        assert_eq!(first_record, three);
        assert_eq!(last_record, four);

        // *** WHERE FILTER + ORDER + PAGINATION TESTING ***

        let records = records_model_query(
            &schema,
            "(where: { type_u8GTE: 7 }, order: {field: TYPE_U8, direction: DESC})",
        )
        .await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let one = connection.edges.get(0).unwrap();
        let two = connection.edges.get(1).unwrap();
        let three = connection.edges.get(2).unwrap();
        assert_eq!(connection.edges.len(), 3);

        let records = records_model_query(
            &schema,
            &format!(
                "(where: {{ type_u8GTE: 7 }}, order: {{field: TYPE_U8, direction: DESC}}, after: \
                 \"{}\")",
                one.cursor
            ),
        )
        .await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        let first_record = connection.edges.first().unwrap();
        assert_eq!(first_record, two);

        let records = records_model_query(
            &schema,
            &format!(
                "(where: {{ type_u8GTE: 7 }}, order: {{field: TYPE_U8, direction: DESC}}, after: \
                 \"{}\")",
                three.cursor
            ),
        )
        .await;
        let connection: Connection<Record> = serde_json::from_value(records).unwrap();
        assert_eq!(connection.edges.len(), 0);

        Ok(())
    }
}

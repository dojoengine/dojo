#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_graphql::value;
    use dojo_types::component::Member;
    use dojo_world::manifest::Component;
    use sqlx::SqlitePool;
    use starknet_crypto::FieldElement;
    use tokio::sync::mpsc;
    use torii_core::sql::Sql;
    use torii_core::State;

    use crate::tests::common::{
        entity_fixtures, run_graphql_query, run_graphql_subscription, Connection, Edge, Moves,
        Position,
    };

    type OrderTestFn = dyn Fn(&Vec<Edge<Position>>) -> bool;

    struct OrderTest {
        direction: &'static str,
        field: &'static str,
        test_order: Box<OrderTestFn>,
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn test_component_no_filter(pool: SqlitePool) {
        entity_fixtures(&pool).await;

        let query = r#"
                {
                    movesComponents {
                        totalCount
                        edges {
                            node {
                                __typename
                                remaining
                            }
                            cursor
                        }
                    }
                    positionComponents {
                        totalCount
                        edges {
                            node {
                                __typename
                                x
                                y
                            }
                            cursor
                        }
                    }
                }
            "#;

        let value = run_graphql_query(&pool, query).await;

        let moves_components = value.get("movesComponents").ok_or("no moves found").unwrap();
        let moves_connection: Connection<Moves> =
            serde_json::from_value(moves_components.clone()).unwrap();

        let position_components =
            value.get("positionComponents").ok_or("no position found").unwrap();
        let position_connection: Connection<Position> =
            serde_json::from_value(position_components.clone()).unwrap();

        assert_eq!(moves_connection.edges[0].node.remaining, 10);
        assert_eq!(position_connection.edges[0].node.x, 42);
        assert_eq!(position_connection.edges[0].node.y, 69);
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn test_component_where_filter(pool: SqlitePool) {
        entity_fixtures(&pool).await;

        // fixtures inserts two position components with members (x: 42, y: 69) and (x: 69, y: 42)
        // the following filters and expected total results can be simply calculated
        let where_filters = Vec::from([
            ("where: { x: 42 }", 1),
            ("where: { xNEQ: 42 }", 1),
            ("where: { xGT: 42 }", 1),
            ("where: { xGTE: 42 }", 2),
            ("where: { xLT: 42 }", 0),
            ("where: { xLTE: 42 }", 1),
            ("where: { x: 1337, yGTE: 1234 }", 0),
        ]);

        for (filter, expected_total) in where_filters {
            let query = format!(
                r#"
                    {{
                        positionComponents ({}) {{
                            totalCount 
                            edges {{
                                node {{
                                    __typename
                                    x
                                    y
                                }}
                                cursor
                            }}
                        }}
                    }}
                "#,
                filter
            );

            let value = run_graphql_query(&pool, &query).await;
            let positions = value.get("positionComponents").ok_or("no positions found").unwrap();
            let connection: Connection<Position> =
                serde_json::from_value(positions.clone()).unwrap();
            assert_eq!(connection.total_count, expected_total);
        }
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn test_component_ordering(pool: SqlitePool) {
        entity_fixtures(&pool).await;

        let orders: Vec<OrderTest> = vec![
            OrderTest {
                direction: "ASC",
                field: "X",
                test_order: Box::new(|edges: &Vec<Edge<Position>>| {
                    edges[0].node.x < edges[1].node.x
                }),
            },
            OrderTest {
                direction: "DESC",
                field: "X",
                test_order: Box::new(|edges: &Vec<Edge<Position>>| {
                    edges[0].node.x > edges[1].node.x
                }),
            },
            OrderTest {
                direction: "ASC",
                field: "Y",
                test_order: Box::new(|edges: &Vec<Edge<Position>>| {
                    edges[0].node.y < edges[1].node.y
                }),
            },
            OrderTest {
                direction: "DESC",
                field: "Y",
                test_order: Box::new(|edges: &Vec<Edge<Position>>| {
                    edges[0].node.y > edges[1].node.y
                }),
            },
        ];

        for order in orders {
            let query = format!(
                r#"
                {{
                    positionComponents (order: {{ direction: {}, field: {} }}) {{
                        totalCount
                        edges {{
                            node {{
                                __typename
                                x
                                y
                            }}
                            cursor
                        }}
                    }}
                }}
            "#,
                order.direction, order.field
            );

            let value = run_graphql_query(&pool, &query).await;
            let positions = value.get("positionComponents").ok_or("no positions found").unwrap();
            let connection: Connection<Position> =
                serde_json::from_value(positions.clone()).unwrap();
            assert_eq!(connection.total_count, 2);
            assert!((order.test_order)(&connection.edges));
        }
    }

    #[sqlx::test(migrations = "../migrations")]
    async fn test_component_entity_relationship(pool: SqlitePool) {
        entity_fixtures(&pool).await;

        let query = r#"
                {
                    positionComponents {
                        totalCount 
                        edges {
                            node {
                                __typename
                                x
                                y
                                entity {
                                    keys
                                    componentNames
                                }
                            }
                            cursor
                        }
                    }
                }
            "#;
        let value = run_graphql_query(&pool, query).await;

        let positions = value.get("positionComponents").ok_or("no positions found").unwrap();
        let connection: Connection<Position> = serde_json::from_value(positions.clone()).unwrap();
        let entity = connection.edges[0].node.entity.as_ref().unwrap();
        assert_eq!(entity.component_names, "Position".to_string());
    }
    #[sqlx::test(migrations = "../migrations")]
    async fn test_component_subscription(pool: SqlitePool) {
        // Sleep in order to run this test at the end in a single thread
        tokio::time::sleep(Duration::from_secs(30)).await;

        let state = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        // 0. Preprocess component value
        let name = "Test".to_string();
        let component_id = name.to_lowercase();
        let class_hash = FieldElement::TWO;
        let hex_class_hash = format!("{:#x}", class_hash);
        let expected_value: async_graphql::Value = value!({
         "ComponentAdded": { "id": component_id.clone(), "name":name, "classHash": hex_class_hash }
        });
        let (tx, mut rx) = mpsc::channel(7);

        tokio::spawn(async move {
            // 1. Open process and sleep.Go to execute subscription
            tokio::time::sleep(Duration::from_secs(1)).await;

            let component = Component {
                name,
                members: vec![Member { name: "test".into(), ty: "u32".into(), key: false }],
                class_hash,
                ..Default::default()
            };
            state.register_component(component).await.unwrap();
            // 3. fn publish() is called from state.set_entity()

            tx.send(()).await.unwrap();
        });

        // 2. The subscription is executed and it is listeing, waiting for publish() to be executed
        let response_value = run_graphql_subscription(
            &pool,
            r#"
						subscription {
							ComponentAdded {
									id, name, classHash
								}
						}"#,
        )
        .await;
        // 4. The subcription has received the message from publish()
        // 5. Compare values
        assert_eq!(expected_value, response_value);
        rx.recv().await.unwrap();
    }
}

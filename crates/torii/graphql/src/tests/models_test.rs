#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;
    use starknet_crypto::FieldElement;
    use torii_core::sql::Sql;

    use crate::tests::{entity_fixtures, run_graphql_query, Connection, Edge, Moves, Position};

    type OrderTestFn = dyn Fn(&Vec<Edge<Position>>) -> bool;

    struct OrderTest {
        direction: &'static str,
        field: &'static str,
        test_order: Box<OrderTestFn>,
    }
    //#[ignore]
    #[sqlx::test(migrations = "../migrations")]
    async fn test_model_no_filter(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();

        entity_fixtures(&mut db).await;

        let query = r#"
                {
                    movesModels {
                        total_count
                        edges {
                            node {
                                __typename
                                remaining
                                last_direction
                            }
                            cursor
                        }
                    }
                    positionModels {
                        total_count
                        edges {
                            node {
                                __typename
																vec {
																	x
																	y
																}
                            }
                            cursor
                        }
                    }
                }
            "#;

        let value = run_graphql_query(&pool, query).await;

        let moves_models = value.get("movesModels").ok_or("no moves found").unwrap();
        let moves_connection: Connection<Moves> =
            serde_json::from_value(moves_models.clone()).unwrap();

        let position_models = value.get("positionModels").ok_or("no position found").unwrap();
        let position_connection: Connection<Position> =
            serde_json::from_value(position_models.clone()).unwrap();

        assert_eq!(moves_connection.edges[0].node.remaining, 10);
        assert_eq!(position_connection.edges[0].node.vec.x, 42);
        assert_eq!(position_connection.edges[0].node.vec.y, 69);
    }

    //#[ignore]
    #[sqlx::test(migrations = "../migrations")]
    async fn test_model_where_filter(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();

        entity_fixtures(&mut db).await;

        // fixtures inserts two position mdoels with members (x: 42, y: 69) and (x: 69, y: 42)
        // the following filters and expected total results can be simply calculated
        let where_filters = Vec::from([
            (
                r#"where: { playerNEQ: "0x0000000000000000000000000000000000000000000000000000000000000002" }"#,
                1,
            ),
            (
                r#"where: { playerGT: "0x0000000000000000000000000000000000000000000000000000000000000002" }"#,
                1,
            ),
            (
                r#"where: { playerGTE: "0x0000000000000000000000000000000000000000000000000000000000000002" }"#,
                2,
            ),
            (
                r#"where: { playerLT: "0x0000000000000000000000000000000000000000000000000000000000000002" }"#,
                0,
            ),
            (
                r#"where: { playerLTE: "0x0000000000000000000000000000000000000000000000000000000000000002" }"#,
                1,
            ),
            (
                r#"where: { player: "0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973" }"#,
                0,
            ),
            (
                r#"where: { player: "0x0000000000000000000000000000000000000000000000000000000000000002" }"#,
                1,
            ), // player is a key
        ]);

        for (filter, expected_total) in where_filters {
            let query = format!(
                r#"
                        {{
                            positionModels ({}) {{
                                total_count
                                edges {{
                                    node {{
                                        __typename
                                        vec {{
																					x
																					y
																				}}
                                    }}
                                    cursor
                                }}
                            }}
                        }}
                    "#,
                filter
            );

            let value = run_graphql_query(&pool, &query).await;
            let positions = value.get("positionModels").ok_or("no positions found").unwrap();
            let connection: Connection<Position> =
                serde_json::from_value(positions.clone()).unwrap();
            assert_eq!(connection.total_count, expected_total);
        }
    }

    #[ignore]
    #[sqlx::test(migrations = "../migrations")]
    // Todo: reenable OrderTest struct(test_order field)
    // Todo: Refactor fn fetch_multiple_rows(), external_field
    async fn test_model_ordering(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();

        entity_fixtures(&mut db).await;

        let orders: Vec<OrderTest> = vec![
            OrderTest {
                direction: "ASC",
                field: "X", //"PLAYER"
                test_order: Box::new(|edges: &Vec<Edge<Position>>| {
                    edges[0].node.vec.x < edges[1].node.vec.x
                }),
            },
            OrderTest {
                direction: "DESC",
                field: "X",
                test_order: Box::new(|edges: &Vec<Edge<Position>>| {
                    edges[0].node.vec.x > edges[1].node.vec.x
                }),
            },
            OrderTest {
                direction: "ASC",
                field: "Y", //"PLAYER"
                test_order: Box::new(|edges: &Vec<Edge<Position>>| {
                    edges[0].node.vec.y < edges[1].node.vec.y
                }),
            },
            OrderTest {
                direction: "DESC",
                field: "Y",
                test_order: Box::new(|edges: &Vec<Edge<Position>>| {
                    edges[0].node.vec.y > edges[1].node.vec.y
                }),
            },
        ];

        for order in orders {
            let query = format!(
                r#"
                    {{
                        positionModels (order: {{ direction: {}, field: {} }}) {{
                            total_count
                            edges {{
                                node {{
                                    __typename
                                    vec {{
																			x
																			y
																		}}
                                }}
                                cursor
                            }}
                        }}
                    }}
                "#,
                order.direction, order.field
            );

            let value = run_graphql_query(&pool, &query).await;
            let positions = value.get("positionModels").ok_or("no positions found").unwrap();
            let connection: Connection<Position> =
                serde_json::from_value(positions.clone()).unwrap();
            assert_eq!(connection.total_count, 2);
            assert!((order.test_order)(&connection.edges));
        }
    }

    //#[ignore]
    #[sqlx::test(migrations = "../migrations")]
    async fn test_model_entity_relationship(pool: SqlitePool) {
        let mut db = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();

        entity_fixtures(&mut db).await;

        // Todo: Add `keys` field on `entity` type
        // fixme: `keys` field return a single string, but test expects vec of strings
        let query = r#"
                    {
                        positionModels {
                            total_count
                            edges {
                                node {
                                    __typename
                                    vec {
																			x
																			y
																		}
                                    entity {
                                        model_names
                                    }
                                }
                                cursor
                            }
                        }
                    }
                "#;
        let value = run_graphql_query(&pool, query).await;

        let positions = value.get("positionModels").ok_or("no positions found").unwrap();
        let connection: Connection<Position> = serde_json::from_value(positions.clone()).unwrap();
        let entity = connection.edges[0].node.entity.as_ref().unwrap();
        assert_eq!(entity.model_names, "Moves,Position".to_string());
    }
}

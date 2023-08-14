#[cfg(test)]
mod tests {
    use sqlx::SqlitePool;

    use crate::tests::common::{entity_fixtures, run_graphql_query, Connection, Moves, Position};

    #[sqlx::test(migrations = "./migrations")]
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

    // TODO: add back in filter after WhereInput
    // #[sqlx::test(migrations = "./migrations")]
    // async fn test_component_filter(pool: SqlitePool) {
    //     entity_fixtures(&pool).await;

    //     let query = r#"
    //             {
    //                 positionComponents (x: 42) {
    //                     __typename
    //                     x
    //                     y
    //                 }
    //             }
    //         "#;
    //     let value = run_graphql_query(&pool, query).await;

    //     let positions = value.get("positionComponents").ok_or("no positions found").unwrap();
    //     let positions: Vec<Position> = serde_json::from_value(positions.clone()).unwrap();
    //     assert_eq!(positions[0].y, 69);
    // }

    #[sqlx::test(migrations = "./migrations")]
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
        assert_eq!(entity.component_names, "Moves,Position".to_string());
    }
}

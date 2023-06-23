#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use chrono::{DateTime, Utc};
    use serde::Deserialize;
    use sqlx::{FromRow, SqlitePool};
    use starknet::core::types::FieldElement;
    use starknet_crypto::poseidon_hash_many;

    use crate::state::sql::{Executable, Sql};
    use crate::state::State;
    use crate::tests::common::run_graphql_query;

    #[derive(Deserialize)]
    struct Moves {
        __typename: String,
        remaining: u32,
    }

    #[derive(Deserialize)]
    struct Position {
        __typename: String,
        x: u32,
        y: u32,
    }

    #[derive(FromRow, Deserialize)]
    pub struct Component {
        pub id: String,
        pub name: String,
        pub class_hash: String,
        pub transaction_hash: String,
        pub created_at: DateTime<Utc>,
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_storage_by_key(pool: SqlitePool) {
        entity_fixtures(&pool).await;

        let moves_key = poseidon_hash_many(&[FieldElement::ONE]);
        let position_key = poseidon_hash_many(&[FieldElement::TWO]);
        let query = format!(
            r#"
                {{
                    moves (key: "{:#x}") {{
                        __typename
                        remaining
                    }}
                    position (key: "{:#x}") {{
                        __typename
                        x
                        y
                    }}
                }}
            "#,
            moves_key, position_key
        );
        let value = run_graphql_query(&pool, query.as_str()).await;

        let moves = value.get("moves").ok_or("no moves found").unwrap();
        let moves: Moves = serde_json::from_value(moves.clone()).unwrap();
        let position = value.get("position").ok_or("no position found").unwrap();
        let position: Position = serde_json::from_value(position.clone()).unwrap();

        assert_eq!(moves.remaining, 10);
        assert_eq!(position.x, 42);
        assert_eq!(position.y, 69);
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn test_storage_filter(pool: SqlitePool) {
        entity_fixtures(&pool).await;

        let query = r#"
                {
                    positionList (x: 42) {
                        __typename
                        x
                        y
                    }
                }
            "#;
        let value = run_graphql_query(&pool, query).await;

        let positions = value.get("positionList").ok_or("no positions found").unwrap();
        let positions: Vec<Position> = serde_json::from_value(positions.clone()).unwrap();

        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].y, 69);
    }

    // TODO: relationship between component and storage needs to be rethought,
    // storage (component instance) should be really nested under entity

    // #[sqlx::test(migrations = "./migrations")]
    // async fn test_storage_union(pool: SqlitePool) {
    //     component_fixtures(&pool).await;

    //     let query = r#"
    //             {
    //                 component_moves: component(id: "moves") {
    //                     name
    //                     storage {
    //                         __typename
    //                         ... on Moves {
    //                             remaining
    //                         }
    //                     }
    //                 }
    //                 component_position: component(id: "position") {
    //                     name
    //                     storage {
    //                         __typename
    //                         ... on Position {
    //                             x
    //                             y
    //                         }
    //                     }
    //                 }
    //             }
    //         "#;
    //     let value = run_graphql_query(&pool, query).await;
    //     let component_moves = value.get("component_moves").ok_or("no component found").unwrap();
    //     let component_moves: ComponentMoves =
    //         serde_json::from_value(component_moves.clone()).unwrap();
    //     let component_position =
    //         value.get("component_position").ok_or("no component found").unwrap();
    //     let component_position: ComponentPosition =
    //         serde_json::from_value(component_position.clone()).unwrap();

    //     assert_eq!(component_moves.name, component_moves.storage.__typename);
    //     assert_eq!(component_position.name, component_position.storage.__typename);
    // }

    async fn entity_fixtures(pool: &SqlitePool) {
        let manifest = dojo_world::manifest::Manifest::load_from_path(
            Utf8PathBuf::from_path_buf("../../examples/ecs/target/dev/manifest.json".into())
                .unwrap(),
        )
        .unwrap();

        let state = Sql::new(pool.clone(), FieldElement::ZERO).await.unwrap();
        state.load_from_manifest(manifest).await.unwrap();

        // Set moves entity
        let key = vec![FieldElement::ONE];
        let partition = FieldElement::from_hex_be("0xdead").unwrap();
        let values = vec![FieldElement::from_hex_be("0xa").unwrap()];
        state.set_entity("moves".to_string(), partition, key, values).await.unwrap();

        // Set position entity
        let key = vec![FieldElement::TWO];
        let partition = FieldElement::from_hex_be("0xbeef").unwrap();
        let values = vec![
            FieldElement::from_hex_be("0x2a").unwrap(),
            FieldElement::from_hex_be("0x45").unwrap(),
        ];
        state.set_entity("position".to_string(), partition, key, values).await.unwrap();
        state.execute().await.unwrap();
    }
}

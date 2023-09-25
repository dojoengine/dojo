use starknet::macros::felt;
use wasm_bindgen_test::wasm_bindgen_test;

#[wasm_bindgen_test]
async fn test_spawn_client() {
    let url = "http://localhost:5000";
    let world_address = "0x398c6b4f479e2a6181ae895ad34333b44e419e48098d2a9622f976216d044dd";
    let initial_entities_to_sync =
        serde_wasm_bindgen::to_value(&dojo_types::component::EntityComponent {
            component: "Position".into(),
            keys: vec![felt!("0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973")],
        })
        .map(|v| vec![v])
        .unwrap();

    assert!(
        torii_client_wasm::spawn_client(url, world_address, initial_entities_to_sync).await.is_ok(),
        "failed to spawn client"
    )
}

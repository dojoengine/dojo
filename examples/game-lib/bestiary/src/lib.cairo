#[derive(Drop, Serde)]
#[dojo::model(namespace: "bestiary")]
struct RiverSkale {
    #[key]
    id: u32,
    health: u32,
    armor: u32,
    attack: u32,
}

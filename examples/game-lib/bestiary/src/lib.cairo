#[derive(Drop, Serde)]
#[dojo::model(namespace: "bestiary")]
pub struct RiverSkale {
    #[key]
    pub id: u32,
    pub health: u32,
    pub armor: u32,
    pub attack: u32,
}

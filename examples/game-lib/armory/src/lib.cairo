#[derive(Drop, Serde)]
#[dojo::model(namespace: "armory")]
pub struct Flatbow {
    #[key]
    pub id: u32,
    pub atk_speek: u32,
    pub range: u32,
}

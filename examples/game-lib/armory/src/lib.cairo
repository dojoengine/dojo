#[derive(Drop, Serde)]
#[dojo::model(namespace: "armory")]
struct Flatbow {
    #[key]
    id: u32,
    atk_speek: u32,
    range: u32,
}

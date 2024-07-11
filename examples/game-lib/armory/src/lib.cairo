#[derive(Drop, Serde)]
#[dojo::model]
struct Flatbow {
    #[key]
    id: u32,
    atk_speek: u32,
    range: u32,
}

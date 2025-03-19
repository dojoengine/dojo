#[derive(Clone, Debug)]
pub struct Resource {
    pub name: &'static str,
}

pub fn get_resources() -> Vec<Resource> {
    vec![] // Add resources as needed
}

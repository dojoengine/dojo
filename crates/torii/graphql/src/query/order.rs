use strum_macros::{AsRefStr, EnumString};

#[derive(AsRefStr, Debug, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Direction {
    Asc,
    Desc,
}

#[derive(Debug)]
pub struct Order {
    pub field: String,
    pub direction: Direction,
}

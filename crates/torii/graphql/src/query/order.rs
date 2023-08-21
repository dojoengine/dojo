use core::fmt;

#[derive(Debug)]
pub enum Direction {
    Asc,
    Desc,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Direction::Asc => write!(f, "ASC"),
            Direction::Desc => write!(f, "DESC"),
        }
    }
}

impl TryFrom<&str> for Direction {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "ASC" => Ok(Direction::Asc),
            "DESC" => Ok(Direction::Desc),
            _ => Err(format!("Invalid direction: {}", value)),
        }
    }
}

#[derive(Debug)]
pub struct Order {
    pub field: String,
    pub direction: Direction,
}

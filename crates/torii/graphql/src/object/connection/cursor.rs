use async_graphql::Error;
use base64::engine::general_purpose;
use base64::Engine as _;

pub fn encode(primary: &str, secondary: &str) -> String {
    let cursor = format!("cursor/{}/{}", primary, secondary);
    general_purpose::STANDARD.encode(cursor.as_bytes())
}

pub fn decode(cursor: &str) -> Result<(String, String), Error> {
    let bytes = general_purpose::STANDARD.decode(cursor)?;
    let cursor = String::from_utf8(bytes)?;
    let parts: Vec<&str> = cursor.split('/').collect();

    if parts.len() != 3 || parts[0] != "cursor" {
        return Err("Invalid cursor format".into());
    }

    let primary = parts[1].parse::<String>()?;
    let secondary = parts[2].parse::<String>()?;

    Ok((primary, secondary))
}

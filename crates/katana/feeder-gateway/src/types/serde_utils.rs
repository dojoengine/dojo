use serde::de::Visitor;
use serde::Deserialize;

pub fn deserialize_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct U64HexVisitor;

    impl<'de> Visitor<'de> for U64HexVisitor {
        type Value = u64;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "0x-prefix hex string or decimal number")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if let Some(hex) = v.strip_prefix("0x") {
                u64::from_str_radix(hex, 16).map_err(serde::de::Error::custom)
            } else {
                v.parse::<u64>().map_err(serde::de::Error::custom)
            }
        }
    }

    deserializer.deserialize_any(U64HexVisitor)
}

pub fn deserialize_u128<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct U128HexVisitor;

    impl<'de> Visitor<'de> for U128HexVisitor {
        type Value = u128;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "0x-prefix hex string or decimal number")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if let Some(hex) = v.strip_prefix("0x") {
                u128::from_str_radix(hex, 16).map_err(serde::de::Error::custom)
            } else {
                v.parse::<u128>().map_err(serde::de::Error::custom)
            }
        }
    }

    deserializer.deserialize_any(U128HexVisitor)
}

pub fn deserialize_optional_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNum {
        String(String),
        Number(u64),
    }

    match Option::<StringOrNum>::deserialize(deserializer)? {
        None => Ok(None),
        Some(StringOrNum::Number(n)) => Ok(Some(n)),
        Some(StringOrNum::String(s)) => {
            if let Some(hex) = s.strip_prefix("0x") {
                u64::from_str_radix(hex, 16).map(Some).map_err(serde::de::Error::custom)
            } else {
                s.parse().map(Some).map_err(serde::de::Error::custom)
            }
        }
    }
}

pub fn deserialize_optional_u128<'de, D>(deserializer: D) -> Result<Option<u128>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNum {
        String(String),
        Number(u128),
    }

    match Option::<StringOrNum>::deserialize(deserializer)? {
        None => Ok(None),
        Some(StringOrNum::Number(n)) => Ok(Some(n)),
        Some(StringOrNum::String(s)) => {
            if let Some(hex) = s.strip_prefix("0x") {
                u128::from_str_radix(hex, 16).map(Some).map_err(serde::de::Error::custom)
            } else {
                s.parse().map(Some).map_err(serde::de::Error::custom)
            }
        }
    }
}

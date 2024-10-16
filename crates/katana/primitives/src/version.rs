/// The currently supported version of the Starknet protocol.
pub const CURRENT_STARKNET_VERSION: ProtocolVersion = ProtocolVersion::new([0, 13, 1, 1]); // version 0.13.1.1

// TODO: figure out the exact format of the version string.
/// Starknet protocol version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolVersion {
    /// Each segments represents a part of the version number.
    segments: [u8; 4],
}

#[derive(Debug, thiserror::Error)]
pub enum ParseVersionError {
    #[error("invalid version format")]
    InvalidFormat,
    #[error("failed to parse segment: {0}")]
    ParseSegment(#[from] std::num::ParseIntError),
}

impl ProtocolVersion {
    pub const fn new(segments: [u8; 4]) -> Self {
        Self { segments }
    }

    /// Parses a version string in the format `x.y.z.w` where x, y, z, w are u8 numbers.
    /// The string can have fewer than 4 segments; missing segments are filled with zeros.
    pub fn parse(version: &str) -> Result<Self, ParseVersionError> {
        let segments = version.split('.').collect::<Vec<&str>>();

        if segments.is_empty() || segments.len() > 4 {
            return Err(ParseVersionError::InvalidFormat);
        }

        let mut buffer = [0u8; 4];
        for (buf, seg) in buffer.iter_mut().zip(segments) {
            *buf = seg.parse::<u8>()?;
        }

        Ok(Self::new(buffer))
    }
}

impl core::default::Default for ProtocolVersion {
    fn default() -> Self {
        ProtocolVersion::new([0, 1, 0, 0])
    }
}

// Formats the version as a string, where each segment is separated by a dot.
// The last segment (fourth part) will not be printed if it's zero.
//
// For example:
// - Version::new([1, 2, 3, 4]) will be displayed as "1.2.3.4"
// - Version::new([1, 2, 3, 0]) will be displayed as "1.2.3"
// - Version::new([0, 2, 3, 0]) will be displayed as "0.2.3"
impl core::fmt::Display for ProtocolVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for (idx, segment) in self.segments.iter().enumerate() {
            // If it's the last segment, don't print it if it's zero.
            if idx == self.segments.len() - 1 {
                if *segment != 0 {
                    write!(f, ".{segment}")?;
                }
            } else if idx == 0 {
                write!(f, "{segment}")?;
            } else {
                write!(f, ".{segment}")?;
            }
        }

        Ok(())
    }
}

impl TryFrom<String> for ProtocolVersion {
    type Error = ParseVersionError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        ProtocolVersion::parse(&value)
    }
}

#[cfg(feature = "serde")]
mod serde {
    use super::*;

    // We de/serialize the version from/into a human-readable string format to prevent breaking the
    // database encoding format if ever decide to change its memory representation.

    impl ::serde::Serialize for ProtocolVersion {
        fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.serialize_str(&self.to_string())
        }
    }

    impl<'de> ::serde::Deserialize<'de> for ProtocolVersion {
        fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            let s = String::deserialize(deserializer)?;
            ProtocolVersion::parse(&s).map_err(::serde::de::Error::custom)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_valid() {
        let version = "1.9.0.0";
        let parsed = ProtocolVersion::parse(version).unwrap();
        assert_eq!(parsed.segments, [1, 9, 0, 0]);
        assert_eq!(String::from("1.9.0"), parsed.to_string());
    }

    #[test]
    fn parse_version_missing_parts() {
        let version = "1.9.0";
        assert!(ProtocolVersion::parse(version).is_err());
    }

    #[test]
    fn parse_version_invalid_digit_should_fail() {
        let version = "0.fv.1.0";
        assert!(ProtocolVersion::parse(version).is_err());
    }

    #[test]
    fn parse_version_missing_digit_should_fail() {
        let version = "1...";
        assert!(ProtocolVersion::parse(version).is_err());
    }

    #[test]
    fn parse_version_many_parts_should_succeed() {
        let version = "1.2.3.4";
        let parsed = ProtocolVersion::parse(version).unwrap();
        assert_eq!(parsed.segments, [1, 2, 3, 4]);
        assert_eq!(String::from("1.2.3.4"), parsed.to_string());
    }

    #[cfg(feature = "serde")]
    mod serde {
        use super::*;

        #[test]
        fn rt_human_readable() {
            let version = ProtocolVersion::new([1, 2, 3, 4]);
            let serialized = serde_json::to_string(&version).unwrap();
            let deserialized: ProtocolVersion = serde_json::from_str(&serialized).unwrap();
            assert_eq!(version, deserialized);
        }

        #[test]
        fn rt_non_human_readable() {
            let version = ProtocolVersion::new([1, 2, 3, 4]);
            let serialized = postcard::to_stdvec(&version).unwrap();
            let deserialized: ProtocolVersion = postcard::from_bytes(&serialized).unwrap();
            assert_eq!(version, deserialized);
        }
    }
}

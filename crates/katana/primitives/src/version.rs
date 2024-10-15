/// The currently supported version of the Starknet protocol.
pub const CURRENT_STARKNET_VERSION: Version = Version::new([0, 13, 1, 1]); // version 0.13.1.1

/// Starknet protocol version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    segments: [u8; 4],
}

impl Version {
    pub const fn new(segments: [u8; 4]) -> Self {
        Self { segments }
    }

    pub fn parse(version: &str) -> anyhow::Result<Self> {
        let segments: Result<Vec<u8>, _> = version.split('.').map(|s| s.parse::<u8>()).collect();

        match segments {
            Ok(segments) if segments.len() == 4 => {
                let mut arr = [0u8; 4];
                arr.copy_from_slice(&segments);
                Ok(Self { segments: arr })
            }
            _ => Err(anyhow::anyhow!("invalid version format")),
        }
    }
}

impl std::default::Default for Version {
    fn default() -> Self {
        Version::new([0, 1, 0, 0])
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, segment) in self.segments.iter().enumerate() {
            if i == self.segments.len() - 1 {
                if i != 0 {
                    write!(f, ".{segment}")?;
                }
            } else if i == 0 {
                write!(f, "{segment}")?;
            } else {
                write!(f, ".{segment}")?;
            }
        }

        Ok(())
    }
}

#[cfg(feature = "serde")]
mod serde {
    use super::*;

    impl ::serde::Serialize for Version {
        fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            if serializer.is_human_readable() {
                serializer.serialize_str(&self.to_string())
            } else {
                self.segments.serialize(serializer)
            }
        }
    }

    impl<'de> ::serde::Deserialize<'de> for Version {
        fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            if deserializer.is_human_readable() {
                let s = String::deserialize(deserializer)?;
                Version::parse(&s).map_err(::serde::de::Error::custom)
            } else {
                Ok(Version::new(<[u8; 4]>::deserialize(deserializer)?))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_valid() {
        let version = "1.9.0.0";
        let parsed = Version::parse(version).unwrap();
        assert_eq!(parsed.segments, [1, 9, 0, 0]);
        assert_eq!(String::from("1.9.0.0"), parsed.to_string());
    }

    #[test]
    fn parse_version_missing_parts() {
        let version = "1.9.0";
        assert!(Version::parse(version).is_err());
    }

    #[test]
    fn parse_version_invalid_digit_should_fail() {
        let version = "0.fv.1.0";
        assert!(Version::parse(version).is_err());
    }

    #[test]
    fn parse_version_missing_digit_should_fail() {
        let version = "1...";
        assert!(Version::parse(version).is_err());
    }

    #[test]
    fn parse_version_many_parts_should_succeed() {
        let version = "1.2.3.4";
        let parsed = Version::parse(version).unwrap();
        assert_eq!(parsed.segments, [1, 2, 3, 4]);
        assert_eq!(String::from("1.2.3.4"), parsed.to_string());
    }

    #[cfg(feature = "serde")]
    mod serde {
        use super::*;

        #[test]
        fn rt_human_readable() {
            let version = Version::new([1, 2, 3, 4]);
            let serialized = serde_json::to_string(&version).unwrap();
            assert_eq!(serialized, "\"1.2.3.4\"");
            let deserialized: Version = serde_json::from_str(&serialized).unwrap();
            assert_eq!(version, deserialized);
        }

        #[test]
        fn rt_non_human_readable() {
            let version = Version::new([1, 2, 3, 4]);
            let serialized = postcard::to_stdvec(&version).unwrap();
            let deserialized: Version = postcard::from_bytes(&serialized).unwrap();
            assert_eq!(version, deserialized);
        }
    }
}

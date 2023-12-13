use anyhow::anyhow;

/// The currently supported version of the Starknet protocol.
pub static CURRENT_STARKNET_VERSION: Version = Version::new(0, 12, 2); // version 0.12.2

/// Starknet protocol version.
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(::serde::Serialize, ::serde::Deserialize))]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    pub const fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self { major, minor, patch }
    }

    pub fn parse(version: &str) -> anyhow::Result<Self> {
        let mut parts = version.split('.');

        if parts.clone().count() > 3 {
            return Err(anyhow!("invalid version format"));
        }

        let major = parts.next().map(|s| s.parse::<u64>()).transpose()?.unwrap_or_default();
        let minor = parts.next().map(|s| s.parse::<u64>()).transpose()?.unwrap_or_default();
        let patch = parts.next().map(|s| s.parse::<u64>()).transpose()?.unwrap_or_default();

        Ok(Self::new(major, minor, patch))
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{major}.{minor}.{patch}",
            major = self.major,
            minor = self.minor,
            patch = self.patch
        )
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn parse_semver_valid() {
        let version = "1.9.0";
        let parsed = super::Version::parse(version).unwrap();
        assert_eq!(parsed.major, 1);
        assert_eq!(parsed.minor, 9);
        assert_eq!(parsed.patch, 0);
        assert_eq!(String::from("1.9.0"), parsed.to_string());
    }

    #[test]
    fn parse_semver_missing_parts() {
        let version = "1.9";
        let parsed = super::Version::parse(version).unwrap();
        assert_eq!(parsed.major, 1);
        assert_eq!(parsed.minor, 9);
        assert_eq!(parsed.patch, 0);
        assert_eq!(String::from("1.9.0"), parsed.to_string());
    }

    #[test]
    fn parse_semver_invalid_digit_should_fail() {
        let version = "0.fv.1";
        assert!(super::Version::parse(version).is_err());
    }

    #[test]
    fn parse_semver_missing_digit_should_fail() {
        let version = "1..";
        assert!(super::Version::parse(version).is_err());
    }

    #[test]
    fn parse_semver_too_many_parts_should_fail() {
        let version = "1.2.3.4";
        assert!(super::Version::parse(version).is_err());
    }
}

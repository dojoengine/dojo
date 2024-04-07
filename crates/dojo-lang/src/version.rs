use anyhow::Result;
use semver::Version;

pub trait ToVersion {
    fn to_version(self) -> Result<Version>;
}

impl ToVersion for Version {
    fn to_version(self) -> Result<Version> {
        Ok(self)
    }
}

impl ToVersion for &str {
    fn to_version(self) -> Result<Version> {
        Version::parse(self.trim())
            .map_err(|_| anyhow::format_err!("cannot parse '{}' as a semver", self))
    }
}

impl ToVersion for &String {
    fn to_version(self) -> Result<Version> {
        (**self).to_version()
    }
}

impl ToVersion for &Version {
    fn to_version(self) -> Result<Version> {
        Ok(self.clone())
    }
}

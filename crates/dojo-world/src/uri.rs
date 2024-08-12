use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use url::Url;

#[derive(Debug)]
pub enum UriParseError {
    InvalidUri,
    InvalidFileUri,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Uri {
    Http(Url),
    Ipfs(String),
    File(PathBuf),
}

impl Serialize for Uri {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Uri::Http(url) => serializer.serialize_str(url.as_ref()),
            Uri::Ipfs(ipfs) => serializer.serialize_str(ipfs),
            Uri::File(path) => serializer.serialize_str(&format!("file://{}", path.display())),
        }
    }
}

impl<'de> Deserialize<'de> for Uri {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Uri::from_string(&s).map_err(serde::de::Error::custom)
    }
}

impl Uri {
    pub fn cid(&self) -> Option<&str> {
        match self {
            Uri::Ipfs(value) => value.strip_prefix("ipfs://"),
            _ => None,
        }
    }

    pub fn from_string(s: &str) -> Result<Self> {
        if s.starts_with("ipfs://") {
            Ok(Uri::Ipfs(s.to_string()))
        } else if let Some(path) = s.strip_prefix("file://") {
            Ok(Uri::File(PathBuf::from(&path)))
        } else if let Ok(url) = Url::parse(s) {
            Ok(Uri::Http(url))
        } else {
            Err(anyhow::anyhow!("Invalid Uri"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uri_serialize() {
        let http_uri = Uri::Http(Url::parse("http://example.com").unwrap());
        let ipfs_uri =
            Uri::Ipfs("ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG".to_string());
        let file_uri = Uri::File(PathBuf::from("/path/to/file"));

        assert_eq!(serde_json::to_string(&http_uri).unwrap(), "\"http://example.com/\"");
        assert_eq!(
            serde_json::to_string(&ipfs_uri).unwrap(),
            "\"ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG\""
        );
        assert_eq!(serde_json::to_string(&file_uri).unwrap(), "\"file:///path/to/file\"");
    }

    #[test]
    fn test_uri_deserialize() {
        let http_uri: Uri = serde_json::from_str("\"http://example.com\"").unwrap();
        let ipfs_uri: Uri =
            serde_json::from_str("\"ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG\"")
                .unwrap();
        let file_uri: Uri = serde_json::from_str("\"file:///path/to/file\"").unwrap();

        assert!(matches!(http_uri, Uri::Http(_)));
        assert!(matches!(ipfs_uri, Uri::Ipfs(_)));
        assert!(matches!(file_uri, Uri::File(_)));
    }

    #[test]
    fn test_uri_cid() {
        let ipfs_uri =
            Uri::Ipfs("ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG".to_string());
        let http_uri = Uri::Http(Url::parse("http://example.com").unwrap());

        assert_eq!(ipfs_uri.cid(), Some("QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG"));
        assert_eq!(http_uri.cid(), None);
    }

    #[test]
    fn test_uri_from_str() {
        let ipfs_str = "ipfs://QmYwAPJzv5CZsnA625s3Xf2nemtYgPpHdWEz79ojWnPbdG";
        let file_str = "file:///path/to/file";
        let http_str = "http://example.com";
        let invalid_str = "invalid_uri";

        assert!(matches!(Uri::from_string(ipfs_str).unwrap(), Uri::Ipfs(_)));
        assert!(matches!(Uri::from_string(file_str).unwrap(), Uri::File(_)));
        assert!(matches!(Uri::from_string(http_str).unwrap(), Uri::Http(_)));
        assert!(Uri::from_string(invalid_str).is_err());
    }
}

use anyhow::{anyhow, Result};
use cainome::cairo_serde::{ByteArray, CairoSerde};
use regex::Regex;
use starknet::core::types::Felt;
use starknet_crypto::poseidon_hash_many;

pub const CONTRACT_NAME_SEPARATOR: &str = "::";
pub const TAG_SEPARATOR: char = '-';
pub const SELECTOR_CHUNK_SIZE: usize = 8;

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub fn get_name_from_tag(tag: &str) -> String {
    let parts: Vec<&str> = tag.split(TAG_SEPARATOR).collect();
    parts.last().unwrap().to_string()
}

pub fn get_namespace_from_tag(tag: &str) -> String {
    let parts: Vec<&str> = tag.split(TAG_SEPARATOR).collect();
    parts.first().unwrap().to_string()
}

pub fn get_tag(namespace: &str, name: &str) -> String {
    format!("{namespace}{TAG_SEPARATOR}{name}")
}

pub fn is_valid_tag(tag: &str) -> bool {
    let (namespace, name) = match split_tag(tag) {
        Ok((nm, n)) => (nm, n),
        Err(_) => return false,
    };

    is_name_valid(&namespace) && is_name_valid(&name)
}

/// Check if the provided name follows the format rules for Dojo toolchain.
/// It's not yet enforced at the world level though.
pub fn is_name_valid(name: &str) -> bool {
    Regex::new(r"^[a-zA-Z0-9_]+$").unwrap().is_match(name)
}

/// Get the namespace and the name of a world element from its tag.
pub fn split_tag(tag: &str) -> Result<(String, String)> {
    let parts: Vec<&str> = tag.split(TAG_SEPARATOR).collect();
    match parts.len() {
        2 => Ok((parts[0].to_string(), parts[1].to_string())),
        _ => Err(anyhow!(
            "Unexpected tag. Expected format: <NAMESPACE>{TAG_SEPARATOR}<NAME> or <NAME>"
        )),
    }
}

pub fn get_filename_from_tag(tag: &str) -> String {
    if [format!("dojo{TAG_SEPARATOR}world").as_str(), format!("dojo{TAG_SEPARATOR}base").as_str()]
        .contains(&tag)
    {
        return tag.to_string();
    }

    let mut selector = format!("{:x}", compute_selector_from_tag(tag));
    selector.truncate(SELECTOR_CHUNK_SIZE);

    format!("{tag}{TAG_SEPARATOR}{selector}")
}

pub fn get_tag_from_filename(filename: &str) -> Result<String> {
    let parts: Vec<&str> = filename.split(TAG_SEPARATOR).collect();
    if parts.len() != 3 {
        return Err(anyhow!(
            "Unexpected filename. Expected format: \
             <NAMESPACE>{TAG_SEPARATOR}<NAME>{TAG_SEPARATOR}<SELECTOR>"
        ));
    }

    Ok(format!("{}{TAG_SEPARATOR}{}", parts[0], parts[1]))
}

pub fn compute_bytearray_hash(value: &str) -> Felt {
    let ba = ByteArray::from_string(value).unwrap_or_else(|_| panic!("Invalid ByteArray: {value}"));
    poseidon_hash_many(&ByteArray::cairo_serialize(&ba))
}

pub fn compute_selector_from_tag(tag: &str) -> Felt {
    let (namespace, name) =
        split_tag(tag).unwrap_or_else(|_| panic!("Invalid tag to split: {tag}"));
    compute_selector_from_names(&namespace, &name)
}

pub fn try_compute_selector_from_tag(tag: &str) -> Result<Felt> {
    let (namespace, name) = split_tag(tag)?;
    Ok(compute_selector_from_names(&namespace, &name))
}

pub fn compute_selector_from_tag_or_name(tag_or_name: &str) -> Felt {
    if is_valid_tag(tag_or_name) {
        compute_selector_from_tag(tag_or_name)
    } else {
        compute_bytearray_hash(tag_or_name)
    }
}

pub fn compute_selector_from_names(namespace: &str, name: &str) -> Felt {
    compute_selector_from_hashes(compute_bytearray_hash(namespace), compute_bytearray_hash(name))
}

pub fn compute_selector_from_hashes(namespace_hash: Felt, model_hash: Felt) -> Felt {
    poseidon_hash_many(&[namespace_hash, model_hash])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_tag_success() {
        assert_eq!(
            split_tag("namespace-name").unwrap(),
            ("namespace".to_string(), "name".to_string())
        );
    }

    #[test]
    fn test_split_tag_invalid_chars() {
        assert!(split_tag("invalid:namespace").is_err());
        assert!(split_tag("invalid namespace").is_err());
        assert!(split_tag("inv-alid-namespace").is_err());
    }

    #[test]
    fn test_get_filename_from_tag_success() {
        assert_eq!(get_filename_from_tag("dojo-world"), "dojo-world");
        assert_eq!(get_filename_from_tag("dojo-base"), "dojo-base");

        let tag = "namespace-model";
        let filename = get_filename_from_tag(tag);
        assert!(filename.starts_with(tag));
        assert_eq!(filename.split(TAG_SEPARATOR).count(), 3);
    }

    #[test]
    fn test_compute_bytearray_hash_success() {
        let hash = compute_bytearray_hash("test");
        assert_eq!(
            hash,
            Felt::from_hex("0x2ca96bf6e71766195fa290b97c50f073b218d4e8c6948c899e3b07d754d6760")
                .unwrap()
        );
    }

    #[test]
    fn test_compute_selector_from_tag_success() {
        let selector = compute_selector_from_tag("namespace-model");
        assert_eq!(
            selector,
            Felt::from_hex("0x6cfe11a346c1bb31de8f454d65880454952e22d9adc2374fe67734196e0cbcb")
                .unwrap()
        );
    }

    #[test]
    fn test_compute_selector_from_names_success() {
        let selector = compute_selector_from_names("namespace", "model");
        assert_eq!(
            selector,
            Felt::from_hex("0x6cfe11a346c1bb31de8f454d65880454952e22d9adc2374fe67734196e0cbcb")
                .unwrap()
        );
    }

    #[test]
    fn test_is_valid_tag_success() {
        assert!(is_valid_tag("namespace-model"));
        assert!(is_valid_tag("dojo-world"));
        assert!(is_valid_tag("dojo_examples-base_test"));
    }

    #[test]
    fn test_is_valid_tag_failure() {
        assert!(!is_valid_tag("invalid tag"));
        assert!(!is_valid_tag("invalid@tag"));
        assert!(!is_valid_tag("invalid-"));
        assert!(!is_valid_tag("-invalid"));
        assert!(!is_valid_tag(""));
    }

    #[test]
    fn test_compute_selector_from_tag_or_name_tag() {
        assert_eq!(
            compute_selector_from_tag_or_name("namespace-model"),
            compute_selector_from_tag("namespace-model")
        );
    }

    #[test]
    fn test_compute_selector_from_tag_or_name_name() {
        assert_eq!(
            compute_selector_from_tag_or_name("namespace"),
            compute_bytearray_hash("namespace")
        );
    }

    #[test]
    fn test_try_compute_selector_from_tag_success() {
        assert_eq!(
            try_compute_selector_from_tag("namespace-model").unwrap(),
            compute_selector_from_tag("namespace-model")
        );
    }

    #[test]
    fn test_try_compute_selector_from_tag_failure() {
        assert!(try_compute_selector_from_tag("inv-alid-tag fh").is_err());
    }
}

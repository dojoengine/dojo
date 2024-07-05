use anyhow::{anyhow, Result};
use cainome::cairo_serde::{ByteArray, CairoSerde};
use starknet::core::types::FieldElement;
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

pub fn ensure_namespace(tag: &str, default_namespace: &str) -> String {
    if tag.contains(TAG_SEPARATOR) { tag.to_string() } else { get_tag(default_namespace, tag) }
}

pub fn get_filename_from_tag(tag: &str) -> String {
    if [format!("dojo{TAG_SEPARATOR}world").as_str(), format!("dojo{TAG_SEPARATOR}base").as_str()]
        .contains(&tag)
    {
        return tag.to_string();
    }

    let mut selector = format!("{:x}", compute_model_selector_from_tag(tag));
    selector.truncate(SELECTOR_CHUNK_SIZE);

    format!("{tag}{TAG_SEPARATOR}{selector}")
}

pub fn compute_bytearray_hash(namespace: &str) -> FieldElement {
    let ba = ByteArray::from_string(namespace).unwrap();
    poseidon_hash_many(&ByteArray::cairo_serialize(&ba))
}

pub fn compute_model_selector_from_tag(tag: &str) -> FieldElement {
    let (namespace, name) = split_tag(tag).unwrap();
    compute_model_selector_from_names(&namespace, &name)
}

pub fn compute_model_selector_from_names(namespace: &str, model_name: &str) -> FieldElement {
    compute_model_selector_from_hash(
        compute_bytearray_hash(namespace),
        compute_bytearray_hash(model_name),
    )
}

pub fn compute_model_selector_from_hash(
    namespace_hash: FieldElement,
    model_hash: FieldElement,
) -> FieldElement {
    poseidon_hash_many(&[namespace_hash, model_hash])
}

use anyhow::{anyhow, Result};
use cainome::cairo_serde::{ByteArray, CairoSerde};
use convert_case::{Case, Casing};
use starknet::core::types::FieldElement;
use starknet_crypto::poseidon_hash_many;

pub const FULLNAME_SEPARATOR: char = ':';
pub const MANIFEST_NAME_SEPARATOR: char = '-';

/// The artifact name is used as key to access to some information about
/// compiled elements during the compilation.
/// An artifact name is built by concatenating the fully qualified module name
/// and the element name in snake case, separated by '::'.
///
/// TODO: we don't want to depend on module name, but namespace instead.
pub fn get_artifact_name(module_name: &str, element_name: &str) -> String {
    format!("{module_name}::{element_name}").to_case(Case::Snake)
}

/// Build the full name of an element by concatenating its namespace and its name,
/// using a dedicated separator.
pub fn get_full_world_element_name(namespace: &str, element_name: &str) -> String {
    format!("{}{FULLNAME_SEPARATOR}{}", namespace, element_name)
}

/// Build the full name of an element by concatenating its namespace and its name,
/// using a specific separator.
pub fn get_manifest_name(namespace: &str, element_name: &str) -> String {
    format!(
        "{}{MANIFEST_NAME_SEPARATOR}{}",
        namespace.to_case(Case::Snake),
        element_name.to_case(Case::Snake)
    )
}

/// Get the namespace and the name of a world element from its full name.
/// If no namespace is specified, use the default one.
pub fn split_full_world_element_name(
    full_name: &str,
    default_namespace: &str,
) -> Result<(String, String)> {
    let parts: Vec<&str> = full_name.split(FULLNAME_SEPARATOR).collect();
    match parts.len() {
        1 => Ok((default_namespace.to_string(), full_name.to_string())),
        2 => Ok((parts[0].to_string(), parts[1].to_string())),
        _ => Err(anyhow!(
            "Unexpected full name. Expected format: <NAMESPACE>{FULLNAME_SEPARATOR}<NAME> or \
             <NAME>"
        )),
    }
}

pub fn compute_bytearray_hash(namespace: &str) -> FieldElement {
    let ba = ByteArray::from_string(namespace).unwrap();
    poseidon_hash_many(&ByteArray::cairo_serialize(&ba))
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

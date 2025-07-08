use std::collections::HashSet;
use std::str::FromStr;

use anyhow::{bail, Result};
use clap::{Args, Subcommand};
use dojo_types::naming::{
    compute_bytearray_hash, compute_selector_from_tag, get_name_from_tag, get_namespace_from_tag,
    get_tag,
};
use scarb_metadata::Metadata;
use scarb_metadata_ext::MetadataDojoExt;
use starknet::core::types::Felt;
use starknet::core::utils::{get_selector_from_name, starknet_keccak};
use starknet_crypto::{poseidon_hash_many, poseidon_hash_single};
use tracing::{debug, trace};

#[derive(Debug, Args)]
pub struct HashArgs {
    #[command(subcommand)]
    command: HashCommand,
}

#[derive(Debug, Subcommand)]
pub enum HashCommand {
    #[command(about = "Compute the hash of the provided input.")]
    Compute {
        #[arg(help = "Input to hash. It can be a comma separated list of inputs or a single \
                      input. The single input can be a dojo tag or a felt.")]
        input: String,
    },

    #[command(about = "Search the hash among namespaces and resource names/tags hashes. \
                       Namespaces and resource names can be provided or read from the project \
                       configuration.")]
    Find {
        #[arg(help = "The hash to search for.")]
        hash: String,

        #[arg(short, long)]
        #[arg(value_delimiter = ',')]
        #[arg(help = "Namespaces to use to compute hashes.")]
        namespaces: Option<Vec<String>>,

        #[arg(short, long)]
        #[arg(value_delimiter = ',')]
        #[arg(help = "Resource names to use to compute hashes.")]
        resources: Option<Vec<String>>,
    },
}

impl HashArgs {
    pub fn compute(&self, input: &str) -> Result<()> {
        if input.is_empty() {
            return Err(anyhow::anyhow!("Input is empty"));
        }

        if input.contains('-') {
            let selector = format!("{:#066x}", compute_selector_from_tag(input));
            println!("Dojo selector from tag: {}", selector);
            return Ok(());
        }

        // Selector in starknet is used for types, which must starts with a letter.
        if input.chars().next().is_some_and(|c| c.is_alphabetic()) {
            if input.len() > 32 {
                return Err(anyhow::anyhow!(
                    "Input exceeds the 32-character limit for a Starknet selector"
                ));
            }

            let selector = format!("{:#066x}", get_selector_from_name(input)?);
            let ba_hash = format!("{:#066x}", compute_bytearray_hash(input));

            println!("Starknet selector: {}", selector);
            println!("ByteArray hash: {}", ba_hash);
            return Ok(());
        }

        if !input.contains(',') {
            let felt = Felt::from_str(input)?;
            let poseidon = format!("{:#066x}", poseidon_hash_single(felt));
            let poseidon_array = format!("{:#066x}", poseidon_hash_many(&[felt]));
            let snkeccak = format!("{:#066x}", starknet_keccak(&felt.to_bytes_le()));

            println!("Poseidon single: {}", poseidon);
            println!("Poseidon array 1 value: {}", poseidon_array);
            println!("SnKeccak: {}", snkeccak);

            return Ok(());
        }

        let inputs: Vec<_> = input
            .split(',')
            .map(|s| Felt::from_str(s.trim()).expect("Invalid felt value"))
            .collect();

        let poseidon = format!("{:#066x}", poseidon_hash_many(&inputs));
        println!("Poseidon many: {}", poseidon);

        Ok(())
    }

    pub fn find(
        &self,
        scarb_metadata: &Metadata,
        hash: &String,
        namespaces: Option<Vec<String>>,
        resources: Option<Vec<String>>,
    ) -> Result<()> {
        let hash = Felt::from_str(hash)
            .map_err(|_| anyhow::anyhow!("The provided hash is not valid (hash: {hash})"))?;

        let profile_config = scarb_metadata.load_dojo_profile_config()?;
        let manifest = scarb_metadata.read_dojo_manifest_profile()?;

        let namespaces = namespaces.unwrap_or_else(|| {
            let mut ns_from_config = HashSet::new();

            // get namespaces from profile
            ns_from_config.insert(profile_config.namespace.default);

            if let Some(mappings) = profile_config.namespace.mappings {
                ns_from_config.extend(mappings.into_keys());
            }

            if let Some(models) = &profile_config.models {
                ns_from_config.extend(models.iter().map(|m| get_namespace_from_tag(&m.tag)));
            }

            if let Some(contracts) = &profile_config.contracts {
                ns_from_config.extend(contracts.iter().map(|c| get_namespace_from_tag(&c.tag)));
            }

            if let Some(libraries) = &profile_config.libraries {
                ns_from_config.extend(libraries.iter().map(|c| get_namespace_from_tag(&c.tag)));
            }

            if let Some(events) = &profile_config.events {
                ns_from_config.extend(events.iter().map(|e| get_namespace_from_tag(&e.tag)));
            }

            // get namespaces from manifest
            if let Some(manifest) = &manifest {
                ns_from_config
                    .extend(manifest.models.iter().map(|m| get_namespace_from_tag(&m.tag)));

                ns_from_config
                    .extend(manifest.contracts.iter().map(|c| get_namespace_from_tag(&c.tag)));

                ns_from_config
                    .extend(manifest.events.iter().map(|e| get_namespace_from_tag(&e.tag)));
            }

            Vec::from_iter(ns_from_config)
        });

        let resources = resources.unwrap_or_else(|| {
            let mut res_from_config = HashSet::new();

            // get resources from profile
            if let Some(models) = &profile_config.models {
                res_from_config.extend(models.iter().map(|m| get_name_from_tag(&m.tag)));
            }

            if let Some(contracts) = &profile_config.contracts {
                res_from_config.extend(contracts.iter().map(|c| get_name_from_tag(&c.tag)));
            }

            if let Some(libraries) = &profile_config.libraries {
                res_from_config.extend(libraries.iter().map(|c| get_name_from_tag(&c.tag)));
            }

            if let Some(events) = &profile_config.events {
                res_from_config.extend(events.iter().map(|e| get_name_from_tag(&e.tag)));
            }

            // get resources from manifest
            if let Some(manifest) = &manifest {
                res_from_config.extend(manifest.models.iter().map(|m| get_name_from_tag(&m.tag)));

                res_from_config
                    .extend(manifest.contracts.iter().map(|c| get_name_from_tag(&c.tag)));

                res_from_config.extend(manifest.events.iter().map(|e| get_name_from_tag(&e.tag)));
            }

            Vec::from_iter(res_from_config)
        });

        debug!(namespaces = ?namespaces, "Namespaces");
        debug!(resources = ?resources, "Resources");

        // --- find the hash ---
        let mut hash_found = false;

        // could be a namespace hash
        for ns in &namespaces {
            if hash == compute_bytearray_hash(ns) {
                println!("Namespace found: {ns}");
                hash_found = true;
            }
        }

        // could be a resource name hash
        for res in &resources {
            if hash == compute_bytearray_hash(res) {
                println!("Resource name found: {res}");
                hash_found = true;
            }
        }

        // could be a tag hash (combination of namespace and name)
        for ns in &namespaces {
            for res in &resources {
                let tag = get_tag(ns, res);
                if hash == compute_selector_from_tag(&tag) {
                    println!("Resource tag found: {tag}");
                    hash_found = true;
                }
            }
        }

        if hash_found { Ok(()) } else { bail!("No resource matches the provided hash.") }
    }

    pub fn run(&self, scarb_metadata: &Metadata) -> Result<()> {
        trace!(args = ?self);

        match &self.command {
            HashCommand::Compute { input } => self.compute(input),
            HashCommand::Find { hash, namespaces, resources } => {
                self.find(scarb_metadata, hash, namespaces.clone(), resources.clone())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_dojo_tag() {
        let input = "dojo_examples-actions".to_string();
        let args = HashArgs { command: HashCommand::Compute { input: input.clone() } };
        let result = args.compute(&input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hash_single_felt() {
        let input = "0x1".to_string();
        let args = HashArgs { command: HashCommand::Compute { input: input.clone() } };
        let result = args.compute(&input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hash_starknet_selector() {
        let input = "dojo".to_string();
        let args = HashArgs { command: HashCommand::Compute { input: input.clone() } };
        let result = args.compute(&input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hash_multiple_felts() {
        let input = "0x1,0x2,0x3".to_string();
        let args = HashArgs { command: HashCommand::Compute { input: input.clone() } };
        let result = args.compute(&input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hash_empty_input() {
        let input = "".to_string();
        let args = HashArgs { command: HashCommand::Compute { input: input.clone() } };
        let result = args.compute(&input);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Input is empty");
    }

    #[test]
    fn test_hash_invalid_felt() {
        let input = "invalid too long to be a selector supported by starknet".to_string();
        let args = HashArgs { command: HashCommand::Compute { input: input.clone() } };
        assert!(args.compute(&input).is_err());
    }

    #[test]
    #[should_panic]
    fn test_hash_multiple_invalid_felts() {
        let input = "0x1,0x2,0x3,fhorihgorh".to_string();
        let args = HashArgs { command: HashCommand::Compute { input: input.clone() } };

        let _ = args.compute(&input);
    }
}

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{bail, Result};
use dojo_types::naming;
use serde::Deserialize;
use toml;

use super::environment::Environment;
use super::migration_config::MigrationConfig;
use super::namespace_config::NamespaceConfig;
use super::resource_config::ResourceConfig;
use super::world_config::WorldConfig;

/// External contract configuration for the Profile config.
#[derive(Debug, Clone, Deserialize)]
pub struct ExternalContractConfig {
    pub contract_name: String,
    pub instance_name: Option<String>,
    pub contract_address: Option<String>,
    pub salt: Option<String>,
    pub constructor_data: Option<Vec<String>>,
    pub block_number: Option<u64>,
}

/// Profile configuration that is used to configure the world and its environment.
///
/// This [`ProfileConfig`] is expected to be loaded from a TOML file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProfileConfig {
    pub world: WorldConfig,
    pub models: Option<Vec<ResourceConfig>>,
    pub contracts: Option<Vec<ResourceConfig>>,
    pub libraries: Option<Vec<ResourceConfig>>,
    pub events: Option<Vec<ResourceConfig>>,
    pub external_contracts: Option<Vec<ExternalContractConfig>>,
    pub namespace: NamespaceConfig,
    pub env: Option<Environment>,
    pub migration: Option<MigrationConfig>,
    /// A mapping `<name_or_tag, <tags>>` of writers to be set on the world.
    pub writers: Option<HashMap<String, HashSet<String>>>,
    /// A mapping `<name_or_tag, <tags>>` of owners to be set on the world.
    pub owners: Option<HashMap<String, HashSet<String>>>,
    /// A mapping `<tag, <values>>` of init call arguments to be passed to the contract.
    pub init_call_args: Option<HashMap<String, Vec<String>>>,
    /// A mapping `<tag, version>` of libraries
    pub lib_versions: Option<HashMap<String, String>>,
}

impl ProfileConfig {
    pub fn new(name: &str, seed: &str, namespace: NamespaceConfig) -> Self {
        Self {
            world: WorldConfig {
                name: name.to_string(),
                seed: seed.to_string(),
                ..Default::default()
            },
            namespace,
            ..Default::default()
        }
    }

    /// Loads the profile configuration from a TOML file.
    pub fn from_toml<P: AsRef<Path>>(toml_path: P) -> Result<Self> {
        let content = fs::read_to_string(&toml_path)?;
        let config: ProfileConfig = toml::from_str(&content)?;
        Ok(config)
    }

    /// Returns the local writers for a given tag.
    pub fn get_local_writers(&self, tag: &str) -> HashSet<String> {
        if let Some(writers) = &self.writers {
            writers.get(tag).unwrap_or(&HashSet::new()).clone()
        } else {
            HashSet::new()
        }
    }

    /// Returns the local owners for a given tag.
    pub fn get_local_owners(&self, tag: &str) -> HashSet<String> {
        if let Some(owners) = &self.owners {
            owners.get(tag).unwrap_or(&HashSet::new()).clone()
        } else {
            HashSet::new()
        }
    }

    /// Returns true if the tag has to be skipped during migration.
    pub fn is_skipped(&self, tag: &str) -> bool {
        if let Some(migration) = &self.migration {
            if let Some(skip_contracts) = &migration.skip_contracts {
                skip_contracts.contains(&tag.to_string())
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Validate the consistency of the Profile configuration.
    ///
    /// Rules:
    /// - for a same external contract name we should have:
    ///   + only one external_contracts block if instance name is not set OR,
    ///   + one or several external_contracts blocks with different instance names.
    /// - for a self-managed external contract, the fields instance_name, salt and constructor_data
    ///   should NOT be set.
    pub fn validate(&self) -> Result<()> {
        if let Some(contracts) = &self.external_contracts {
            let mut map = HashMap::<String, Vec<Option<String>>>::new();

            for contract in contracts {
                map.entry(contract.contract_name.clone())
                    .or_default()
                    .push(contract.instance_name.clone());

                // self-managed contract
                if contract.contract_address.is_some()
                    && (contract.instance_name.is_some()
                        || contract.constructor_data.is_some()
                        || contract.salt.is_some())
                {
                    println!(
                        "warning: the contract {} is self-managed so the fields instance_name, \
                         constructor_data and salt should NOT be set",
                        contract.contract_name
                    )
                }
            }

            for (contract_name, instance_names) in map {
                if instance_names.len() > 1 && instance_names.iter().any(|n| n.is_none()) {
                    bail!(
                        "There must be only one [[external_contracts]] block in the profile \
                         config mentioning the contract name '{}' without instance name.",
                        contract_name
                    );
                }

                let instance_name_set: HashSet<_> = instance_names.iter().cloned().collect();
                if instance_name_set.len() != instance_names.len() {
                    bail!(
                        "There are duplicated instance names for the external contract name '{}'",
                        contract_name
                    );
                }

                for instance_name in instance_name_set.into_iter().flatten() {
                    if !naming::is_name_valid(&instance_name) {
                        bail!(
                            "The instance name '{}' is not valid according to Dojo format rules.",
                            instance_name
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use url::Url;

    use super::*;
    use crate::uri::Uri;

    #[test]
    fn test_profile_config_empty() {
        let content = "";

        let error = toml::from_str::<ProfileConfig>(content).unwrap_err();
        assert!(error.to_string().contains("missing field `world`"));
    }

    #[test]
    fn test_profile_config_seed_missing() {
        let content = r#"
        [world]
        name = "test"
        "#;

        let error = toml::from_str::<ProfileConfig>(content).unwrap_err();
        assert!(error.to_string().contains("missing field `seed`"));
    }

    #[test]
    fn test_profile_config_min() {
        let content = r#"
        [world]
        name = "test"
        seed = "abcd"

        [namespace]
        default = "test"
        "#;

        let _ = toml::from_str::<ProfileConfig>(content).unwrap();
    }

    #[test]
    fn test_profile_config_full() {
        let content = r#"
        [world]
        name = "test"
        seed = "abcd"
        description = "test"
        cover_uri = "file://example.com/cover.png"
        icon_uri = "ipfs://example.com/icon.png"
        website = "https://example.com"
        socials = { "twitter" = "test", "discord" = "test" }

        [[models]]
        tag = "ns1-m1"
        description = "This is the m1 model"
        icon_uri = "ipfs://dojo/m1.png"

        [[contracts]]
        tag = "ns1-c1"
        description = "This is the c1 contract"
        icon_uri = "ipfs://dojo/c1.png"

        [[events]]
        tag = "ns1-e1"
        description = "This is the e1 event"
        icon_uri = "ipfs://dojo/e1.png"

        [[external_contracts]]
        contract_name = "ERC1155Token"
        instance_name = "Rewards"
        salt = "1"
        constructor_data = ["0x2af9427c5a277474c079a1283c880ee8a6f0f8fbf73ce969c08d88befec1bba", "str:https://rewards.com/" ]

        [[external_contracts]]
        contract_name = "Saloon"
        block_number = 123

        [[external_contracts]]
        contract_name = "Bank"
        contract_address = "0x2af9427c5a277474c079a1283c880ee8a6f0f8fbf73ce969c08d88befec1bba"

        [namespace]
        default = "test"
        mappings = { "test" = ["test2"] }

        [env]
        rpc_url = "https://example.com/rpc"
        account_address = "test"
        private_key = "test"
        keystore_path = "test"
        keystore_password = "test"
        world_address = "test"

        [env.ipfs_config]
        url = "https://ipfs.service"
        username = "johndoe"
        password = "123456"

        [migration]
        skip_contracts = [ "module::my-contract" ]

        [writers]
        "ns1" = ["ns1-actions"]

        [owners]
        "ns2" = ["ns2-blup"]

        [init_call_args]
        "ns1-actions" = [ "0x1", "0x2" ]

        [lib_versions]
        "ns1-lib" = "0.0.0"
        "#;

        let config = toml::from_str::<ProfileConfig>(content).unwrap();

        let migration = config.migration.unwrap();
        assert_eq!(migration.skip_contracts.unwrap(), vec!["module::my-contract".to_string()]);

        let env = config.env.unwrap();
        assert_eq!(env.rpc_url, Some("https://example.com/rpc".to_string()));
        assert_eq!(env.account_address, Some("test".to_string()));
        assert_eq!(env.private_key, Some("test".to_string()));
        assert_eq!(env.keystore_path, Some("test".to_string()));
        assert_eq!(env.keystore_password, Some("test".to_string()));
        assert_eq!(env.world_address, Some("test".to_string()));

        let ipfs_config = env.ipfs_config.unwrap();
        assert_eq!(ipfs_config.url, "https://ipfs.service".to_string());
        assert_eq!(ipfs_config.username, "johndoe".to_string());
        assert_eq!(ipfs_config.password, "123456".to_string());

        assert_eq!(config.world.description, Some("test".to_string()));
        assert_eq!(
            config.world.cover_uri,
            Some(Uri::from_string("file://example.com/cover.png").unwrap())
        );
        assert_eq!(
            config.world.icon_uri,
            Some(Uri::from_string("ipfs://example.com/icon.png").unwrap())
        );
        assert_eq!(config.world.website, Some(Url::try_from("https://example.com").unwrap()));
        assert_eq!(
            config.world.socials,
            Some(HashMap::from([
                ("twitter".to_string(), "test".to_string()),
                ("discord".to_string(), "test".to_string())
            ]))
        );

        assert!(config.models.is_some());
        let models = config.models.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].tag, "ns1-m1");
        assert_eq!(models[0].description, Some("This is the m1 model".to_string()));
        assert_eq!(models[0].icon_uri, Some(Uri::from_string("ipfs://dojo/m1.png").unwrap()));

        assert!(config.contracts.is_some());
        let contracts = config.contracts.unwrap();
        assert_eq!(contracts.len(), 1);
        assert_eq!(contracts[0].tag, "ns1-c1");
        assert_eq!(contracts[0].description, Some("This is the c1 contract".to_string()));
        assert_eq!(contracts[0].icon_uri, Some(Uri::from_string("ipfs://dojo/c1.png").unwrap()));

        assert!(config.events.is_some());
        let events = config.events.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].tag, "ns1-e1");
        assert_eq!(events[0].description, Some("This is the e1 event".to_string()));
        assert_eq!(events[0].icon_uri, Some(Uri::from_string("ipfs://dojo/e1.png").unwrap()));

        assert!(config.external_contracts.is_some());
        let external_contracts = config.external_contracts.unwrap();

        assert_eq!(external_contracts.len(), 3);
        assert_eq!(external_contracts[0].contract_name, "ERC1155Token");
        assert_eq!(external_contracts[0].instance_name.clone().unwrap(), "Rewards");
        assert_eq!(external_contracts[0].salt.clone().unwrap(), "1");
        assert_eq!(
            external_contracts[0].constructor_data.clone().unwrap(),
            vec![
                "0x2af9427c5a277474c079a1283c880ee8a6f0f8fbf73ce969c08d88befec1bba",
                "str:https://rewards.com/"
            ]
        );
        assert!(external_contracts[0].block_number.is_none());
        assert!(external_contracts[0].contract_address.is_none());

        assert_eq!(external_contracts[1].contract_name, "Saloon");
        assert_eq!(external_contracts[1].block_number.unwrap(), 123);
        assert!(external_contracts[1].contract_address.is_none());
        assert!(external_contracts[1].instance_name.is_none());
        assert!(external_contracts[1].salt.is_none());
        assert!(external_contracts[1].constructor_data.is_none());

        assert_eq!(external_contracts[2].contract_name, "Bank");
        assert_eq!(
            external_contracts[2].contract_address.clone().unwrap(),
            "0x2af9427c5a277474c079a1283c880ee8a6f0f8fbf73ce969c08d88befec1bba"
        );
        assert!(external_contracts[2].block_number.is_none());
        assert!(external_contracts[2].instance_name.is_none());
        assert!(external_contracts[2].salt.is_none());
        assert!(external_contracts[2].constructor_data.is_none());

        assert_eq!(config.namespace.default, "test".to_string());
        assert_eq!(
            config.namespace.mappings,
            Some(HashMap::from([("test".to_string(), vec!["test2".to_string()])]))
        );

        assert_eq!(
            config.writers,
            Some(HashMap::from([("ns1".to_string(), HashSet::from(["ns1-actions".to_string()]))]))
        );
        assert_eq!(
            config.owners,
            Some(HashMap::from([("ns2".to_string(), HashSet::from(["ns2-blup".to_string()]))]))
        );
        assert_eq!(
            config.init_call_args,
            Some(HashMap::from([(
                "ns1-actions".to_string(),
                vec!["0x1".to_string(), "0x2".to_string()]
            )]))
        );
        assert_eq!(
            config.lib_versions,
            Some(HashMap::from([("ns1-lib".to_string(), "0.0.0".to_string())]))
        )
    }

    #[test]
    fn test_profile_config_validation() {
        let mut config = ProfileConfig::new("world", "seed", NamespaceConfig::new("ns"));

        // duplicated None instance name for a same contract name
        config.external_contracts = Some(vec![
            ExternalContractConfig {
                contract_name: "c1".to_string(),
                instance_name: Some("x".to_string()),
                salt: Some("0x01".to_string()),
                constructor_data: None,
                block_number: None,
                contract_address: None,
            },
            ExternalContractConfig {
                contract_name: "c1".to_string(),
                instance_name: None,
                salt: Some("0x01".to_string()),
                constructor_data: None,
                block_number: Some(123),
                contract_address: None,
            },
        ]);

        let res = config.validate();
        assert!(res.is_err());
        assert_eq!(
            res.err().unwrap().to_string(),
            "There must be only one [[external_contracts]] block in the profile config mentioning \
             the contract name 'c1' without instance name."
        );

        // duplicated instance name for a same contract name
        config.external_contracts = Some(vec![
            ExternalContractConfig {
                contract_name: "c1".to_string(),
                instance_name: Some("x".to_string()),
                salt: Some("0x01".to_string()),
                constructor_data: None,
                block_number: None,
                contract_address: None,
            },
            ExternalContractConfig {
                contract_name: "c1".to_string(),
                instance_name: Some("y".to_string()),
                salt: Some("0x01".to_string()),
                constructor_data: None,
                block_number: Some(123),
                contract_address: None,
            },
            ExternalContractConfig {
                contract_name: "c1".to_string(),
                instance_name: Some("x".to_string()),
                salt: Some("0x01".to_string()),
                constructor_data: None,
                block_number: Some(456),
                contract_address: None,
            },
        ]);

        let res = config.validate();
        assert!(res.is_err());
        assert_eq!(
            res.err().unwrap().to_string(),
            "There are duplicated instance names for the external contract name 'c1'"
        );

        // bad instance name
        config.external_contracts = Some(vec![
            ExternalContractConfig {
                contract_name: "c1".to_string(),
                instance_name: None,
                salt: Some("0x01".to_string()),
                constructor_data: None,
                block_number: None,
                contract_address: None,
            },
            ExternalContractConfig {
                contract_name: "c2".to_string(),
                instance_name: Some("x@".to_string()),
                salt: Some("0x01".to_string()),
                constructor_data: None,
                block_number: Some(123),
                contract_address: None,
            },
            ExternalContractConfig {
                contract_name: "c2".to_string(),
                instance_name: Some("y".to_string()),
                salt: Some("0x01".to_string()),
                constructor_data: None,
                block_number: Some(456),
                contract_address: None,
            },
            ExternalContractConfig {
                contract_name: "c3".to_string(),
                instance_name: Some("c3".to_string()),
                salt: Some("0x01".to_string()),
                constructor_data: None,
                block_number: None,
                contract_address: None,
            },
        ]);

        let res = config.validate();
        assert!(res.is_err());
        assert_eq!(
            res.err().unwrap().to_string(),
            "The instance name 'x@' is not valid according to Dojo format rules."
        );

        // nominal case
        config.external_contracts = Some(vec![
            ExternalContractConfig {
                contract_name: "c1".to_string(),
                instance_name: None,
                salt: Some("0x01".to_string()),
                constructor_data: None,
                block_number: None,
                contract_address: None,
            },
            ExternalContractConfig {
                contract_name: "c2".to_string(),
                instance_name: Some("x".to_string()),
                salt: Some("0x01".to_string()),
                constructor_data: None,
                block_number: Some(123),
                contract_address: None,
            },
            ExternalContractConfig {
                contract_name: "c2".to_string(),
                instance_name: Some("y".to_string()),
                salt: Some("0x01".to_string()),
                constructor_data: None,
                block_number: Some(456),
                contract_address: None,
            },
            ExternalContractConfig {
                contract_name: "c3".to_string(),
                instance_name: Some("c3".to_string()),
                salt: Some("0x01".to_string()),
                constructor_data: None,
                block_number: None,
                contract_address: None,
            },
        ]);

        assert!(config.validate().is_ok());
    }
}

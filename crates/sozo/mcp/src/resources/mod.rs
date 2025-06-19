//! Resources for the MCP server.
//!
//! The resources are used to provide information to the client.
//!
//! Sozo doesn't have a database, however, Sozo knows about the project
//! artifacts and files.
use serde::{Deserialize, Serialize};

/// A resource info that is returned when listing the resources.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResourceInfo {
    pub uri: String,
    pub name: String,
    pub title: String,
    pub mime_type: String,
    pub description: String,
}

/// A template that is returned when listing the templates.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TemplateInfo {
    pub uri_template: String,
    pub name: String,
    pub title: String,
    pub mime_type: String,
    pub description: String,
}

#[derive(Debug, Default)]
pub struct ResourceManager {
    resources: Vec<ResourceInfo>,
    templates: Vec<TemplateInfo>,
}

impl ResourceManager {
    /// Creates a new resource manager with all resources available.
    pub fn new() -> Self {
        Self {
            resources: vec![Self::manifest_path()],
            templates: vec![Self::abi(), Self::sierra_class()],
        }
    }

    /// Returns the resources configured for the MCP server.
    pub fn list_resources(&self) -> &[ResourceInfo] {
        &self.resources
    }

    /// Returns the templates configured for the MCP server.
    pub fn list_templates(&self) -> &[TemplateInfo] {
        &self.templates
    }

    /// Returns the resource for the current project manifest path.
    pub fn manifest_path() -> ResourceInfo {
        ResourceInfo {
            uri: "sozo://config/manifest_path".to_string(),
            name: "Current project manifest path".to_string(),
            title: "Current project manifest path".to_string(),
            description: "Current project manifest path, which corresponds to the path of the Scarb.toml file.".to_string(),
            mime_type: "text/plain".to_string(),
        }
    }

    /// Returns the sierra class (if present) for the given contract name.
    ///
    /// The sierra class is garanteed to be present if the project has been
    /// built and the contract tag is valid.
    ///
    /// The contract tag must include the namespace (tag): `ns-contract1`.
    pub fn sierra_class() -> TemplateInfo {
        TemplateInfo {
            uri_template: "sozo://contracts/{{tag}}/sierra_class".to_string(),
            name: "Sierra class for the given contract tag.".to_string(),
            title: "Sierra class for the given contract tag.".to_string(),
            description: "Sierra class for the given contract tag.".to_string(),
            mime_type: "application/json".to_string(),
        }
    }

    /// Returns the ABI for the given contract tag.
    ///
    /// The ABI is garanteed to be present if the project has been
    /// built and the contract tag is valid.
    ///
    /// The contract tag must include the namespace (tag): `ns-contract1`.
    pub fn abi() -> TemplateInfo {
        TemplateInfo {
            uri_template: "sozo://contracts/{{tag}}/abi".to_string(),
            name: "ABI for the given contract tag.".to_string(),
            title: "ABI for the given contract tag.".to_string(),
            description: "ABI for the given contract tag.".to_string(),
            mime_type: "application/json".to_string(),
        }
    }
}

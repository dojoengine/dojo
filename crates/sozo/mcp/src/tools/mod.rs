//! Tools for the MCP server.
//!
//! The profile is usually passed as parameter which gives the possibility
//! to the client to select the profile to use for the action.
//!
//! In the current implementation, the manifest path is passed at the server
//! level, and not configurable at the tool level.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub mod build;
pub mod migrate;
pub mod execute;
pub mod inspect;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Default)]
pub struct ToolManager {
    tools: Vec<Tool>,
}

impl ToolManager {
    /// Creates a new tool manager with all tools available.
    pub fn new() -> Self {
        Self {
            tools: vec![
                Self::build(),
                Self::migrate(),
                Self::execute(),
                Self::inspect(),
            ],
        }
    }

    /// Returns the tools configured for the MCP server.
    pub fn list_tools(&self) -> &[Tool] {
        &self.tools
    }

    /// Returns the tool to build a Dojo project.
    pub fn build() -> Tool {
        Tool {
            name: "build".to_string(),
            description: "Build the project".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "profile": {
                        "type": "string",
                        "description": "Profile to use for build"
                    }
                }
            }),
        }
    }

    /// Returns the tool to migrate the Dojo world and contracts.
    pub fn migrate() -> Tool {
        Tool {
            name: "migrate".to_string(),
            description: "Migrate the Dojo world and contracts using the given profile".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "profile": {
                        "type": "string",
                        "description": "Profile to use for migration"
                    },
                },
                "required": ["profile"]
            }),
        }
    }

    /// Returns the tool to execute a transaction on the blockchain.
    pub fn execute() -> Tool {
        Tool {
            name: "execute".to_string(),
            description: "Execute a transaction on the blockchain".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "function_name": {
                        "type": "string",
                        "description": "Name of the function to call"
                    },
                    "contract_address": {
                        "type": "string",
                        "description": "Address of the target contract"
                    },
                    "calldata": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Calldata for the function"
                    },
                    "profile": {
                        "type": "string",
                        "description": "Profile to use for execution"
                    }
                },
                "required": ["function_name", "contract_address", "calldata", "profile"]
            }),
        }
    }

    /// Returns the tool to inspect the project's contracts, which returns the
    /// list of contracts, their addresses and class hash.
    pub fn inspect() -> Tool {
        Tool {
            name: "inspect".to_string(),
            description: "Get information about the project's contracts".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "profile": {
                        "type": "string",
                        "description": "Profile to use for inspection"
                    }
                },
                "required": ["profile"]
            }),
        }
    }
}

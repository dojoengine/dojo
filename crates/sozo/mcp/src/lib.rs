//! MCP server for Sozo.
//!
//! The current implementation sozo is actually a process that runs the sozo command, and not `sozo_ops` crate.
//! This is not efficient, but limited by the nature of the Scarb's `Config` type.
//!
//! In future versions, this will not be necessary anymore.

use anyhow::Result;
use camino::Utf8PathBuf;
use dojo_world::local::{ResourceLocal, WorldLocal};
use rmcp::{
    Error as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::*,
    service::RequestContext,
    tool, tool_handler, tool_router, transport,
};
use scarb::compiler::Profile;
use scarb::core::Config;
use scarb::ops;
use serde_json::{Value, json};
use smol_str::SmolStr;
use sozo_scarbext::WorkspaceExt;
use std::future::Future;
use tokio::process::Command as AsyncCommand;
use toml;
use tracing::{debug, error};

use crate::tools::{BuildRequest, ExecuteRequest, InspectRequest, MigrateRequest, TestRequest};

mod resources;
mod tools;

const LOG_TARGET: &str = "sozo_mcp";

#[derive(Clone)]
pub struct SozoMcpServer {
    manifest_path: Option<Utf8PathBuf>,
    tool_router: ToolRouter<SozoMcpServer>,
}

#[tool_router]
impl SozoMcpServer {
    pub fn new(manifest_path: Option<Utf8PathBuf>) -> Self {
        Self { manifest_path, tool_router: Self::tool_router() }
    }

    pub async fn serve_stdio(self) -> Result<()> {
        let service = self.serve(transport::stdio()).await.inspect_err(|e| {
            tracing::error!("serving error: {:?}", e);
        })?;

        service.waiting().await?;
        Ok(())
    }

    #[tool(description = "Build the project using the given profile. If no profile is provided, \
                          the default profile `dev` is used.")]
    async fn build(
        &self,
        Parameters(request): Parameters<BuildRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::build::build_project(self.manifest_path.clone(), request).await
    }

    #[tool(description = "Test the project using the given profile. If no profile is provided, \
                          the default profile `dev` is used.")]
    async fn test(
        &self,
        Parameters(request): Parameters<TestRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::test::test_project(self.manifest_path.clone(), request).await
    }

    #[tool(
        description = "Inspect the project to retrieve information about the resources, useful to retrieve models, contracts, events, namespaces, etc."
    )]
    async fn inspect(
        &self,
        Parameters(request): Parameters<InspectRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::inspect::inspect_project(self.manifest_path.clone(), request).await
    }

    #[tool(
        description = "Migrate the project using the given profile. If no profile is provided, \
                          the default profile `dev` is used."
    )]
    async fn migrate(
        &self,
        Parameters(request): Parameters<MigrateRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::migrate::migrate_project(self.manifest_path.clone(), request).await
    }

    #[tool(
        description = "Execute a transaction using the given profile. If no profile is provided, \
                          the default profile `dev` is used."
    )]
    async fn execute(
        &self,
        Parameters(request): Parameters<ExecuteRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::execute::execute_transaction(self.manifest_path.clone(), request).await
    }
}

#[tool_handler]
impl ServerHandler for SozoMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_03_26,
            capabilities: ServerCapabilities::builder().enable_resources().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "This server provides tools to build and migrate a Dojo project using Sozo."
                    .to_string(),
            ),
        }
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParam>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        let resources = vec![
            RawResource {
                uri: "dojo://scarb/manifest".to_string(),
                name: "Scarb project manifest".to_string(),
                description: Some("Scarb project manifest used by Scarb to build the project. This is the file that contains the project's dependencies and configuration.".to_string()),
                mime_type: Some("application/json".to_string()),
                size: None,
            }
            .no_annotation()
        ];

        Ok(ListResourcesResult { resources, next_cursor: None })
    }

    async fn read_resource(
        &self,
        ReadResourceRequestParam { uri }: ReadResourceRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        match uri.as_str() {
            "dojo://scarb/manifest" => {
                // Manifest is a toml file. Will be read and then converted to JSON.
                let manifest_path = self.manifest_path.as_ref().ok_or_else(|| {
                    McpError::resource_not_found(
                        "no_manifest_path",
                        Some(json!({ "reason": "No manifest path provided" })),
                    )
                })?;

                let manifest_json = resources::toml_to_json(manifest_path.clone()).await?;

                Ok(ReadResourceResult {
                    contents: vec![ResourceContents::text(manifest_json, uri)],
                })
            }
            uri if uri.starts_with("dojo://contract/") && uri.ends_with("/abi") => {
                // Extract profile from URI: dojo://contract/{profile}/{name}/abi
                let profile = uri.split("/").nth(1).ok_or_else(|| {
                    McpError::internal_error(
                        "invalid_contract_uri",
                        Some(json!({ "reason": format!("Invalid contract URI: {}", uri) })),
                    )
                })?;

                // Extract contract name from URI: dojo://contract/{profile}/{name}/abi
                let contract_name = uri
                    .strip_prefix(&format!("dojo://contract/{profile}/"))
                    .and_then(|s| s.strip_suffix("/abi"))
                    .ok_or_else(|| {
                        McpError::resource_not_found(
                            "invalid_contract_uri",
                            Some(json!({ "uri": uri })),
                        )
                    })?;

                let world =
                    resources::load_world_local(self.manifest_path.clone(), profile).await?;

                let contract = world
                    .resources
                    .values()
                    .find_map(|r| r.as_contract())
                    .filter(|c| c.common.name == contract_name)
                    .ok_or_else(|| {
                        McpError::resource_not_found(
                            "contract_not_found",
                            Some(json!({ "contract_name": contract_name })),
                        )
                    })?;

                // Convert ABI to JSON
                let abi_json =
                    serde_json::to_string_pretty(&contract.common.class.abi).map_err(|e| {
                        McpError::internal_error(
                            "abi_serialization_failed",
                            Some(json!({ "reason": format!("Failed to serialize ABI: {}", e) })),
                        )
                    })?;

                Ok(ReadResourceResult { contents: vec![ResourceContents::text(abi_json, uri)] })
            }
            uri if uri.starts_with("dojo://model/") && uri.ends_with("/abi") => {
                // Extract model name from URI: dojo://model/{name}/abi
                let model_name = uri
                    .strip_prefix("dojo://model/")
                    .and_then(|s| s.strip_suffix("/abi"))
                    .ok_or_else(|| {
                        McpError::resource_not_found(
                            "invalid_model_uri",
                            Some(json!({ "uri": uri })),
                        )
                    })?;

                // Load world and find the model
                let manifest_path = self.manifest_path.as_ref().ok_or_else(|| {
                    McpError::resource_not_found(
                        "no_manifest_path",
                        Some(json!({ "reason": "No manifest path provided" })),
                    )
                })?;

                let config = Config::builder(manifest_path.clone())
                    .profile(Profile::DEV)
                    .build()
                    .map_err(|e| {
                    McpError::internal_error(
                        "config_build_failed",
                        Some(json!({ "reason": format!("Failed to build config: {}", e) })),
                    )
                })?;

                let ws = ops::read_workspace(config.manifest_path(), &config).map_err(|e| {
                    McpError::internal_error(
                        "workspace_read_failed",
                        Some(json!({ "reason": format!("Failed to read workspace: {}", e) })),
                    )
                })?;

                let world = ws.load_world_local().map_err(|e| {
                    McpError::internal_error(
                        "world_load_failed",
                        Some(json!({ "reason": format!("Failed to load world: {}", e) })),
                    )
                })?;

                let model = world
                    .resources
                    .values()
                    .find_map(|r| match r {
                        ResourceLocal::Model(m) => Some(m),
                        _ => None,
                    })
                    .filter(|m| m.common.name == model_name)
                    .ok_or_else(|| {
                        McpError::resource_not_found(
                            "model_not_found",
                            Some(json!({ "model_name": model_name })),
                        )
                    })?;

                // Convert ABI to JSON
                let abi_json =
                    serde_json::to_string_pretty(&model.common.class.abi).map_err(|e| {
                        McpError::internal_error(
                            "abi_serialization_failed",
                            Some(json!({ "reason": format!("Failed to serialize ABI: {}", e) })),
                        )
                    })?;

                Ok(ReadResourceResult { contents: vec![ResourceContents::text(abi_json, uri)] })
            }
            uri if uri.starts_with("dojo://event/") && uri.ends_with("/abi") => {
                // Extract event name from URI: dojo://event/{name}/abi
                let event_name = uri
                    .strip_prefix("dojo://event/")
                    .and_then(|s| s.strip_suffix("/abi"))
                    .ok_or_else(|| {
                        McpError::resource_not_found(
                            "invalid_event_uri",
                            Some(json!({ "uri": uri })),
                        )
                    })?;

                // Load world and find the event
                let manifest_path = self.manifest_path.as_ref().ok_or_else(|| {
                    McpError::resource_not_found(
                        "no_manifest_path",
                        Some(json!({ "reason": "No manifest path provided" })),
                    )
                })?;

                let config = Config::builder(manifest_path.clone())
                    .profile(Profile::DEV)
                    .build()
                    .map_err(|e| {
                    McpError::internal_error(
                        "config_build_failed",
                        Some(json!({ "reason": format!("Failed to build config: {}", e) })),
                    )
                })?;

                let ws = ops::read_workspace(config.manifest_path(), &config).map_err(|e| {
                    McpError::internal_error(
                        "workspace_read_failed",
                        Some(json!({ "reason": format!("Failed to read workspace: {}", e) })),
                    )
                })?;

                let world = ws.load_world_local().map_err(|e| {
                    McpError::internal_error(
                        "world_load_failed",
                        Some(json!({ "reason": format!("Failed to load world: {}", e) })),
                    )
                })?;

                let event = world
                    .resources
                    .values()
                    .find_map(|r| match r {
                        ResourceLocal::Event(e) => Some(e),
                        _ => None,
                    })
                    .filter(|e| e.common.name == event_name)
                    .ok_or_else(|| {
                        McpError::resource_not_found(
                            "event_not_found",
                            Some(json!({ "event_name": event_name })),
                        )
                    })?;

                // Convert ABI to JSON
                let abi_json =
                    serde_json::to_string_pretty(&event.common.class.abi).map_err(|e| {
                        McpError::internal_error(
                            "abi_serialization_failed",
                            Some(json!({ "reason": format!("Failed to serialize ABI: {}", e) })),
                        )
                    })?;

                Ok(ReadResourceResult { contents: vec![ResourceContents::text(abi_json, uri)] })
            }
            _ => Err(McpError::resource_not_found(
                "resource_not_found",
                Some(json!({
                    "uri": uri
                })),
            )),
        }
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParam>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult { next_cursor: None, prompts: vec![] })
    }

    #[allow(unused_variables)]
    async fn get_prompt(
        &self,
        GetPromptRequestParam { name, arguments }: GetPromptRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        Ok(GetPromptResult { description: None, messages: vec![] })
    }

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParam>,
        _: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        let resource_templates = vec![
            RawResourceTemplate {
                uri_template: "dojo://contract/{profile}/{name}/abi".to_string(),
                name: "Contract ABI".to_string(),
                description: Some(
                    "Get the ABI for a specific contract in the given profile".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
            }
            .no_annotation(),
            RawResourceTemplate {
                uri_template: "dojo://model/{profile}/{name}/abi".to_string(),
                name: "Model ABI".to_string(),
                description: Some(
                    "Get the ABI for a specific model in the given profile".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
            }
            .no_annotation(),
            RawResourceTemplate {
                uri_template: "dojo://event/{profile}/{name}/abi".to_string(),
                name: "Event ABI".to_string(),
                description: Some(
                    "Get the ABI for a specific event in the given profile".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
            }
            .no_annotation(),
        ];

        Ok(ListResourceTemplatesResult { next_cursor: None, resource_templates })
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(self.get_info())
    }
}

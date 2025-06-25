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
use scarb::core::Config;
use scarb::ops;
use scarb::compiler::Profile;
use serde_json::{Value, json};
use smol_str::SmolStr;
use sozo_scarbext::WorkspaceExt;
use std::future::Future;
use tokio::process::Command as AsyncCommand;
use toml;
use tracing::{debug, error};

const LOG_TARGET: &str = "sozo_mcp";

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct BuildRequest {
    #[schemars(description = "Profile to use for build. Default to `dev`.")]
    pub profile: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct TestRequest {
    #[schemars(description = "Profile to use for test. Default to `dev`.")]
    pub profile: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct InspectRequest {
    #[schemars(description = "Profile to use for inspect. Default to `dev`.")]
    pub profile: Option<String>,
}

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

    /// Loads the world local from the manifest path and profile
    async fn load_world_local(&self, profile: &str) -> Result<WorldLocal, McpError> {
        let manifest_path = self.manifest_path.as_ref().ok_or_else(|| {
            McpError::internal_error(
                "no_manifest_path",
                Some(json!({ "reason": "No manifest path provided" })),
            )
        })?;

        let profile_enum = match profile {
            "dev" => Profile::DEV,
            "release" => Profile::RELEASE,
            _ => Profile::new(SmolStr::from(profile)).map_err(|e| {
                McpError::internal_error(
                    "invalid_profile",
                    Some(json!({ "reason": format!("Invalid profile: {}", e) })),
                )
            })?,
        };

        let config =
            Config::builder(manifest_path.clone()).profile(profile_enum).build().map_err(|e| {
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

        Ok(world)
    }

    #[tool(description = "Build the project using the given profile. If no profile is provided, \
                          the default profile `dev` is used.")]
    async fn build(
        &self,
        Parameters(request): Parameters<BuildRequest>,
    ) -> Result<CallToolResult, McpError> {
        let profile = &request.profile.unwrap_or("dev".to_string());

        let mut cmd = AsyncCommand::new("sozo");
        cmd.arg("build");
        cmd.arg("--profile").arg(profile);

        if let Some(manifest_path) = &self.manifest_path {
            cmd.arg("--manifest-path").arg(manifest_path);
        }

        debug!(target: LOG_TARGET, profile, manifest_path = ?self.manifest_path, "Building project.");

        let output = cmd.output().await.map_err(|e| {
            McpError::internal_error(
                "build_failed",
                Some(json!({ "reason": format!("Failed to build project: {}", e) })),
            )
        })?;

        if output.status.success() {
            Ok(CallToolResult::success(vec![Content::text("Build successful".to_string())]))
        } else {
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            Ok(CallToolResult::error(vec![Content::text(err)]))
        }
    }

    #[tool(description = "Test the project using the given profile. If no profile is provided, \
                          the default profile `dev` is used.")]
    async fn test(
        &self,
        Parameters(request): Parameters<TestRequest>,
    ) -> Result<CallToolResult, McpError> {
        let profile = &request.profile.unwrap_or("dev".to_string());

        let mut cmd = AsyncCommand::new("sozo");
        cmd.arg("test");
        cmd.arg("--profile").arg(profile);

        if let Some(manifest_path) = &self.manifest_path {
            cmd.arg("--manifest-path").arg(manifest_path);
        }

        debug!(target: LOG_TARGET, profile, manifest_path = ?self.manifest_path, "Testing project.");

        let output = cmd.output().await.map_err(|e| {
            McpError::internal_error(
                "test_failed",
                Some(json!({ "reason": format!("Failed to test project: {}", e) })),
            )
        })?;

        if output.status.success() {
            Ok(CallToolResult::success(vec![Content::text("Tests passed".to_string())]))
        } else {
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            Ok(CallToolResult::error(vec![Content::text(err)]))
        }
    }

    #[tool(
        description = "Inspect the project to retrieve information about the resources, useful to retrieve models, contracts, events, namespaces, etc."
    )]
    async fn inspect(
        &self,
        Parameters(request): Parameters<InspectRequest>,
    ) -> Result<CallToolResult, McpError> {
        let profile = &request.profile.unwrap_or("dev".to_string());

        let mut cmd = AsyncCommand::new("/Users/glihm/cgg/dojo/target/release/sozo");

        if let Some(manifest_path) = &self.manifest_path {
            cmd.arg("--manifest-path").arg(manifest_path);
        }

        debug!(target: LOG_TARGET, profile, manifest_path = ?self.manifest_path, "Inspecting project.");

        cmd.arg("--profile").arg(profile);
        cmd.arg("inspect");
        cmd.arg("--json");

        let output = cmd.output().await.map_err(|e| {
            McpError::internal_error(
                "inspect_failed",
                Some(json!({ "reason": format!("Failed to inspect project: {}", e) })),
            )
        });

        let output = output?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            match serde_json::from_str::<Value>(&stdout) {
                Ok(json_value) => Ok(CallToolResult::success(vec![Content::json(json_value)?])),
                Err(e) => {
                    error!(target: LOG_TARGET, "Failed to parse JSON: {:?}", e);
                    Ok(CallToolResult::error(vec![Content::text(e.to_string())]))
                }
            }
        } else {
            let err = String::from_utf8_lossy(&output.stderr).to_string();
            error!(target: LOG_TARGET, "Failed to run inspect command: {:?}", err);
            Ok(CallToolResult::error(vec![Content::text(err)]))
        }
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

                let manifest_content =
                    tokio::fs::read_to_string(manifest_path).await.map_err(|e| {
                        McpError::internal_error(
                            "manifest_read_failed",
                            Some(
                                json!({ "reason": format!("Failed to read manifest file: {}", e) }),
                            ),
                        )
                    })?;

                let toml_value: toml::Value = toml::from_str(&manifest_content).map_err(|e| {
                    McpError::internal_error(
                        "manifest_parse_failed",
                        Some(json!({ "reason": format!("Failed to parse TOML: {}", e) })),
                    )
                })?;

                let manifest_json = serde_json::to_string_pretty(&toml_value).map_err(|e| {
                    McpError::internal_error(
                        "manifest_serialization_failed",
                        Some(json!({ "reason": format!("Failed to serialize manifest: {}", e) })),
                    )
                })?;

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

                let world = self.load_world_local(profile).await?;

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
                description: Some("Get the ABI for a specific contract in the given profile".to_string()),
                mime_type: Some("application/json".to_string()),
            }
            .no_annotation(),
            RawResourceTemplate {
                uri_template: "dojo://model/{profile}/{name}/abi".to_string(),
                name: "Model ABI".to_string(),
                description: Some("Get the ABI for a specific model in the given profile".to_string()),
                mime_type: Some("application/json".to_string()),
            }
            .no_annotation(),
            RawResourceTemplate {
                uri_template: "dojo://event/{profile}/{name}/abi".to_string(),
                name: "Event ABI".to_string(),
                description: Some("Get the ABI for a specific event in the given profile".to_string()),
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

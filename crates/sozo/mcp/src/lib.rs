//! MCP server for Sozo.
//!
//! The current implementation sozo is actually a process that runs the sozo command.
//! This is not efficient, but limited by the nature of the Scarb's `Config` type.
//!
//! In future versions, this will not be necessary anymore.

use anyhow::Result;
use camino::Utf8PathBuf;
use rmcp::{
    Error as McpError, RoleServer, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::*,
    service::RequestContext,
    tool, tool_handler, tool_router, transport,
};
use serde_json::{Value, json};
use std::future::Future;
use tokio::process::Command as AsyncCommand;
use tracing::{debug, error};

const LOG_TARGET: &str = "sozo_mcp";

fn _create_resource_text(uri: &str, name: &str) -> Resource {
    RawResource {
        uri: uri.to_string(),
        name: name.to_string(),
        description: Some(name.to_string()),
        mime_type: Some("text/plain".to_string()),
        size: None,
    }
    .no_annotation()
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

    #[tool(description = "Build the project using the given profile. If no profile is provided, \
                          the default profile `dev` is used.")]
    async fn build(
        &self,
        Parameters(object): Parameters<JsonObject>,
    ) -> Result<CallToolResult, McpError> {
        let profile = object.get("profile").and_then(|v| v.as_str()).unwrap_or("dev");

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

    #[tool(
        description = "Inspect the project to retrieve information about the resources, useful to retrieve models, contracts, events, namespaces, etc."
    )]
    async fn inspect(
        &self,
        Parameters(object): Parameters<JsonObject>,
    ) -> Result<CallToolResult, McpError> {
        let profile = object.get("profile").and_then(|v| v.as_str()).unwrap_or("dev");

        let mut cmd = AsyncCommand::new("/Users/glihm/cgg/dojo/target/release/sozo");

        if let Some(manifest_path) = &self.manifest_path {
            cmd.arg("--manifest-path").arg(manifest_path);
        }

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
        Ok(ListResourcesResult {
            resources: vec![
                _create_resource_text("str:////Users/to/some/path/", "cwd"),
                _create_resource_text("memo://insights", "memo-name"),
            ],
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        ReadResourceRequestParam { uri }: ReadResourceRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        match uri.as_str() {
            "str:////Users/to/some/path/" => {
                let cwd = "/Users/to/some/path/";
                Ok(ReadResourceResult { contents: vec![ResourceContents::text(cwd, uri)] })
            }
            "memo://insights" => {
                let memo = "Business Intelligence Memo\n\nAnalysis has revealed 5 key insights ...";
                Ok(ReadResourceResult { contents: vec![ResourceContents::text(memo, uri)] })
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
        Ok(ListResourceTemplatesResult { next_cursor: None, resource_templates: Vec::new() })
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(self.get_info())
    }
}

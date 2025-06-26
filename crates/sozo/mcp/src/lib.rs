//! MCP server for Sozo.
//!
//! The current implementation sozo is actually a process that runs the sozo command, and not
//! `sozo_ops` crate. This is not efficient, but limited by the nature of the Scarb's `Config` type.
//!
//! In future versions, this will not be necessary anymore.

use std::future::Future;

use anyhow::Result;
use camino::Utf8PathBuf;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::tool::Parameters;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{
    tool, tool_handler, tool_router, transport, Error as McpError, RoleServer, ServerHandler,
    ServiceExt,
};

use crate::tools::{BuildRequest, ExecuteRequest, InspectRequest, MigrateRequest, TestRequest};

mod resources;
mod tools;

const LOG_TARGET: &str = "sozo_mcp";

#[derive(Clone, Debug)]
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

    #[tool(description = "Inspect the project to retrieve information about the resources, \
                          useful to retrieve models, contracts, events, namespaces, etc.")]
    async fn inspect(
        &self,
        Parameters(request): Parameters<InspectRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::inspect::inspect_project(self.manifest_path.clone(), request).await
    }

    #[tool(description = "Migrate the project using the given profile. If no profile is \
                          provided, the default profile `dev` is used.")]
    async fn migrate(
        &self,
        Parameters(request): Parameters<MigrateRequest>,
    ) -> Result<CallToolResult, McpError> {
        tools::migrate::migrate_project(self.manifest_path.clone(), request).await
    }

    #[tool(description = "Execute a transaction using the given profile. If no profile is \
                          provided, the default profile `dev` is used.")]
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
            instructions: Some(include_str!("../INSTRUCTIONS.md").to_string()),
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
                description: Some(
                    "Scarb project manifest used by Scarb to build the project. This is the file \
                     that contains the project's dependencies and configuration."
                        .to_string(),
                ),
                mime_type: Some("application/json".to_string()),
                size: None,
            }
            .no_annotation(),
        ];

        Ok(ListResourcesResult { resources, next_cursor: None })
    }

    async fn read_resource(
        &self,
        ReadResourceRequestParam { uri }: ReadResourceRequestParam,
        _: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        resources::handle_resource(&uri, self.manifest_path.clone()).await
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

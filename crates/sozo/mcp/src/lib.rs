//! MCP server for Sozo.
//!
//! The current implementation sozo is actually a process that runs the sozo command.
//! This is not efficient, but limited by the nature of the Scarb's `Config` type.
//!
//! In future versions, this will not be necessary anymore.

use std::env;
use std::io::{self, BufRead, BufReader, Write};

use anyhow::Result;
use axum::{
    Router,
    extract::{Json, State},
    response::Json as JsonResponse,
    routing::{get, post},
};
use camino::Utf8PathBuf;
use clap::Args;
use itertools::Itertools;
use scarb::core::Config;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::process::Command as AsyncCommand;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::resources::ResourceManager;
use crate::tools::{Tool, ToolManager};

mod resources;
mod tools;

const LOG_TARGET: &str = "sozo_mcp_server";
const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

#[derive(Debug, Serialize, Deserialize)]
struct McpRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct McpResponse {
    jsonrpc: String,
    id: Option<Value>,
    result: Option<Value>,
    error: Option<McpError>,
}

impl McpResponse {
    pub fn new_ok(id: Option<Value>, result: Value) -> Self {
        Self { jsonrpc: "2.0".to_string(), id, result: Some(result), error: None }
    }

    pub fn new_error(id: Option<Value>, error: McpError) -> Self {
        Self { jsonrpc: "2.0".to_string(), id, result: None, error: Some(error) }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct McpError {
    code: i32,
    message: String,
    data: Option<Value>,
}

pub struct SozoMcpServer {
    tools_manager: ToolManager,
    resources_manager: ResourceManager,
    state: AppState,
}

#[derive(Debug, Clone)]
pub struct AppState {
    manifest_path: Option<Utf8PathBuf>,
}

#[derive(Debug, Clone)]
pub enum ServerMode {
    Http { port: u16 },
    Stdio,
}

impl SozoMcpServer {
    pub fn new(manifest_path: Option<Utf8PathBuf>) -> Self {
        let state = AppState { manifest_path };

        Self { tools_manager: ToolManager::new(), resources_manager: ResourceManager::new(), state }
    }

    pub async fn start(&self, mode: ServerMode) -> Result<()> {
        match mode {
            ServerMode::Http { port } => self.start_http(port).await,
            ServerMode::Stdio => self.start_stdio().await,
        }
    }

    pub async fn start_stdio(&self) -> Result<()> {
        info!(target: LOG_TARGET, "Starting MCP server in STDIO mode");
        
        let stdin = io::stdin();
        let mut stdout = io::stdout();
        let reader = BufReader::new(stdin);

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let request: McpRequest = serde_json::from_str(&line)?;
            
            let response = self.handle_request(request, self.state.clone()).await;
            
            let response_json = serde_json::to_string(&response)?;
            writeln!(stdout, "{}", response_json)?;
            stdout.flush()?;
        }

        Ok(())
    }

    pub async fn start_http(&self, port: u16) -> Result<()> {
        let cors = CorsLayer::new().allow_methods(Any).allow_origin(Any);

        let app = Router::new()
            .route("/", post(Self::handle_mcp_request))
            .route("/health", get(Self::health_check))
            .layer(cors)
            .with_state(self.state.clone());

        let address = format!("127.0.0.1:{}", port);
        let listener = TcpListener::bind(&address).await?;

        info!(target: LOG_TARGET, address, "MCP Server starting in HTTP mode.");

        // Start the server
        axum::serve(listener, app).await?;

        Ok(())
    }

    // Health check endpoint.
    async fn health_check() -> JsonResponse<Value> {
        JsonResponse(json!({
            "status": "healthy",
            "service": "sozo-mcp-server",
            "version": "1.0.0",
        }))
    }

    // Main MCP request handler for JSON-RPC.
    async fn handle_mcp_request(
        state: State<AppState>,
        Json(request): Json<McpRequest>,
    ) -> JsonResponse<McpResponse> {
        let server = SozoMcpServer::new(state.manifest_path.clone());
        let response = server.handle_request(request, state.0.clone()).await;
        JsonResponse(response)
    }

    async fn handle_request(&self, request: McpRequest, state: AppState) -> McpResponse {
        match request.method.as_str() {
            "tools/list" => self.handle_tools_list(request.id),
            "tools/call" => self.handle_tools_call(request.id, request.params, state.clone()).await,
            "resources/list" => self.handle_resources_list(request.id),
            "resources/templates/list" => self.handle_resources_templates_list(request.id),
            "resources/read" => self.handle_resources_read(request.id, request.params).await,
            "initialize" => self.handle_initialize(request.id),
            _ => McpResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(McpError {
                    code: -32601,
                    message: "Method not found".to_string(),
                    data: None,
                }),
            },
        }
    }

    fn handle_initialize(&self, id: Option<Value>) -> McpResponse {
        McpResponse::new_ok(
            id,
            json!({
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {
                    "tools": {},
                    "resources": {}
                },
                "serverInfo": {
                    "name": "sozo-mcp",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }),
        )
    }

    fn handle_tools_list(&self, id: Option<Value>) -> McpResponse {
        McpResponse::new_ok(
            id,
            json!({
                "tools": self.tools_manager.list_tools()
            }),
        )
    }

    async fn handle_tools_call(
        &self,
        id: Option<Value>,
        params: Option<Value>,
        state: AppState,
    ) -> McpResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: "Invalid params".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let tool_name = params["name"].as_str().unwrap_or("");
        let arguments = &params["arguments"];

        let result = match tool_name {
            "migrate" => tools::migrate::migrate(arguments).await,
            "execute" => tools::execute::execute_transaction(arguments).await,
            "inspect" => tools::inspect::inspect(arguments).await,
            "build" => tools::build::build_project(arguments, state.clone()).await,
            _ => Err("Unknown tool".to_string()),
        };

        match result {
            Ok(output) => McpResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "content": [{
                        "type": "text",
                        "text": output
                    }]
                })),
                error: None,
            },
            Err(error) => McpResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(McpError { code: -32603, message: error, data: None }),
            },
        }
    }

    fn handle_resources_list(&self, id: Option<Value>) -> McpResponse {
        McpResponse::new_ok(
            id,
            json!({
                "resources": self.resources_manager.list_resources()
            }),
        )
    }

    fn handle_resources_templates_list(&self, id: Option<Value>) -> McpResponse {
        McpResponse::new_ok(
            id,
            json!({
                "resourceTemplates": self.resources_manager.list_templates()
            }),
        )
    }

    async fn handle_resources_read(&self, id: Option<Value>, params: Option<Value>) -> McpResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: "Invalid params".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let uri = params["uri"].as_str().unwrap_or("");

        let content = match uri {
            "aa" => Ok("TODO"),
            /* "sozo://config/manifest_path" => self.get_manifest_path().await,
            "sozo://contracts/{{tag}}/sierra_class" => self.get_sierra_class(uri).await, */
            _ => Err("Unknown resource".to_string()),
        };

        match content {
            Ok(data) => McpResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "text/plain",
                        "text": data
                    }]
                })),
                error: None,
            },
            Err(error) => McpResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(McpError { code: -32603, message: error, data: None }),
            },
        }
    }
}

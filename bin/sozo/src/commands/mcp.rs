//! MCP server for Sozo.
//!
//! The current implementation sozo is actually a process that runs the sozo command.
//! This is not efficient, but limited by the nature of the Scarb's `Config` type.
//!
//! In future versions, this will not be necessary anymore.

use std::env;

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

use crate::args::ProfileSpec;

#[derive(Debug, Clone, Args)]
pub struct McpArgs {
    #[arg(long, default_value = "10300")]
    #[arg(help = "Port to start the MCP server on.")]
    pub port: u16,
}

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

#[derive(Debug, Serialize, Deserialize)]
struct McpError {
    code: i32,
    message: String,
    data: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Tool {
    name: String,
    description: String,
    input_schema: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct Resource {
    uri: String,
    name: String,
    description: String,
    mime_type: Option<String>,
}

pub struct SozoMcpServer {
    tools: Vec<Tool>,
    resources: Vec<Resource>,
    state: AppState,
}

#[derive(Debug, Clone)]
pub struct AppState {
    manifest_path: Option<Utf8PathBuf>,
}

impl McpArgs {
    pub fn run(
        self,
        config: &Config,
        manifest_path: Option<Utf8PathBuf>,
    ) -> Result<()> {
        let app_state = AppState { manifest_path };

        config.tokio_handle().block_on(async {
            let server = SozoMcpServer::new(app_state);
            server.start(self.port).await?;

            Ok(())
        })
    }
}

impl SozoMcpServer {
    pub fn new(app_state: AppState) -> Self {
        let tools = vec![
            Tool {
                name: "migrate".to_string(),
                description: "Migrate the Dojo world and contracts using the given profile"
                    .to_string(),
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
            },
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
            },
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
            },
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
            },
        ];

        let resources = vec![
            Resource {
                uri: "sozo://logs/recent".to_string(),
                name: "Recent Sozo Logs".to_string(),
                description: "Recent transaction logs and outputs".to_string(),
                mime_type: Some("text/plain".to_string()),
            },
            Resource {
                uri: "sozo://config/current".to_string(),
                name: "Current Configuration".to_string(),
                description: "Current Sozo configuration settings".to_string(),
                mime_type: Some("application/json".to_string()),
            },
        ];

        Self { tools, resources, state: app_state }
    }

    pub async fn start(&self, port: u16) -> Result<()> {
        // Configure CORS - use Any to allow all methods
        let cors = CorsLayer::new().allow_methods(Any).allow_origin(Any);

        // Create the router
        let app = Router::new()
            .route("/", post(Self::handle_mcp_request))
            .route("/health", get(Self::health_check))
            .route("/tools", get(Self::list_tools))
            .route("/resources", get(Self::list_resources))
            .layer(cors)
            .with_state(self.state.clone());

        // Create the server
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await?;

        println!("🚀 Sozo MCP Server starting on http://127.0.0.1:{}", port);
        println!("📋 Available endpoints:");
        println!("   POST / - MCP JSON-RPC endpoint");
        println!("   GET  /health - Health check");
        println!("   GET  /tools - List available tools");
        println!("   GET  /resources - List available resources");

        // Start the server
        axum::serve(listener, app).await?;

        Ok(())
    }

    // Health check endpoint
    async fn health_check() -> JsonResponse<Value> {
        JsonResponse(json!({
            "status": "healthy",
            "service": "sozo-mcp-server",
            "version": "1.0.0"
        }))
    }

    // List tools endpoint
    async fn list_tools(state: State<AppState>) -> JsonResponse<Value> {
        let server = SozoMcpServer::new(state.0);
        JsonResponse(json!({
            "tools": server.tools
        }))
    }

    // List resources endpoint
    async fn list_resources(state: State<AppState>) -> JsonResponse<Value> {
        let server = SozoMcpServer::new(state.0);
        JsonResponse(json!({
            "resources": server.resources
        }))
    }

    // Main MCP request handler.
    async fn handle_mcp_request(
        state: State<AppState>,
        Json(request): Json<McpRequest>,
    ) -> JsonResponse<McpResponse> {
        let server = SozoMcpServer::new(state.0.clone());
        let response = server.handle_request(request, state.0.clone()).await;
        JsonResponse(response)
    }

    async fn handle_request(&self, request: McpRequest, state: AppState) -> McpResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "tools/list" => self.handle_tools_list(request.id),
            "tools/call" => self.handle_tools_call(request.id, request.params, state.clone()).await,
            "resources/list" => self.handle_resources_list(request.id),
            "resources/read" => self.handle_resources_read(request.id, request.params).await,
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
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {
                    "tools": {},
                    "resources": {}
                },
                "serverInfo": {
                    "name": "sozo-mcp-server",
                    "version": "1.0.0"
                }
            })),
            error: None,
        }
    }

    fn handle_tools_list(&self, id: Option<Value>) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "tools": self.tools
            })),
            error: None,
        }
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
            "migrate" => self.execute_migrate(arguments).await,
            "execute" => self.execute_transaction(arguments).await,
            "inspect" => self.get_contract_info(arguments).await,
            "build" => self.build_project(arguments, state.clone()).await,
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
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "resources": self.resources
            })),
            error: None,
        }
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
            "sozo://logs/recent" => self.get_recent_logs().await,
            "sozo://config/current" => self.get_current_config().await,
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

    // Sozo command implementations
    async fn execute_migrate(&self, args: &Value) -> Result<String, String> {
        let profile = args["profile"].as_str().ok_or("Missing profile")?;

        let mut cmd = AsyncCommand::new("sozo");
        cmd.arg("migrate").arg("--profile").arg(profile);

        let output =
            cmd.output().await.map_err(|e| format!("Failed to execute sozo migrate: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    async fn execute_transaction(&self, args: &Value) -> Result<String, String> {
        let function_name = args["function_name"].as_str().ok_or("Missing function_name")?;
        let contract_address =
            args["contract_address"].as_str().ok_or("Missing contract_address")?;
        let calldata = args["calldata"].as_array().ok_or("Missing calldata")?;
        let calldata = calldata.iter().map(|x| x.as_str().unwrap_or("")).join(" ");
        let profile = args["profile"].as_str().unwrap_or("dev");

        let mut cmd = AsyncCommand::new("sozo");
        cmd.arg("execute").arg("--profile").arg(profile).arg(contract_address).arg(function_name).arg(calldata);

        if let Some(calldata) = args["calldata"].as_array() {
            for param in calldata {
                if let Some(param_str) = param.as_str() {
                    cmd.arg(param_str);
                }
            }
        }

        let output =
            cmd.output().await.map_err(|e| format!("Failed to execute sozo transaction: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    async fn get_contract_info(&self, args: &Value) -> Result<String, String> {
        let profile = args["profile"].as_str().unwrap_or("dev");

        let output = AsyncCommand::new("sozo")
            .arg("inspect")
            .arg("--profile")
            .arg(profile)
            .output()
            .await
            .map_err(|e| format!("Failed to get contract info: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    async fn build_project(&self, args: &Value, state: AppState) -> Result<String, String> {
        let profile = args["profile"].as_str().unwrap_or("dev");
        
        let mut cmd = AsyncCommand::new("sozo");
        cmd.arg("build").arg("--profile").arg(profile);
        
        // Add manifest path if provided
        if let Some(manifest_path) = state.manifest_path {
            cmd.arg("--manifest-path").arg(manifest_path);
        }

        let output = cmd.output().await.map_err(|e| format!("Failed to build project: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }

    async fn get_recent_logs(&self) -> Result<String, String> {
        // This would typically read from a log file or command output
        Ok("Recent Sozo logs would be displayed here".to_string())
    }

    async fn get_current_config(&self) -> Result<String, String> {
        let output = AsyncCommand::new("sozo")
            .arg("config")
            .arg("--show")
            .output()
            .await
            .map_err(|e| format!("Failed to get config: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).to_string())
        }
    }
}

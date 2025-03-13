use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::broadcast;

// Constants
pub const JSONRPC_VERSION: &str = "2.0";
pub const MCP_VERSION: &str = "2024-11-05";
pub const SSE_CHANNEL_CAPACITY: usize = 100;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Value,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcNotification {
    pub _jsonrpc: String,
    pub _method: String,
    pub _params: Option<Value>,
}

#[derive(Debug, Serialize, Clone)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Clone)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct Implementation {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct ServerCapabilities {
    pub tools: ToolCapabilities,
    pub resources: ResourceCapabilities,
}

#[derive(Debug, Serialize)]
pub struct ToolCapabilities {
    pub list_changed: bool,
}

#[derive(Debug, Serialize)]
pub struct ResourceCapabilities {
    pub subscribe: bool,
    pub list_changed: bool,
}

// Structure to hold SSE session information
#[derive(Clone, Debug)]
pub struct SseSession {
    pub tx: broadcast::Sender<JsonRpcResponse>,
    pub _session_id: String,
}

impl JsonRpcResponse {
    pub fn ok(id: Value, result: Value) -> Self {
        Self { jsonrpc: JSONRPC_VERSION.to_string(), id, result: Some(result), error: None }
    }

    pub fn error(id: Value, code: i32, message: &str, data: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message: message.to_string(), data }),
        }
    }

    pub fn invalid_request(id: Value) -> Self {
        Self::error(id, -32600, "Invalid Request", None)
    }

    pub fn method_not_found(id: Value) -> Self {
        Self::error(id, -32601, "Method not found", None)
    }

    pub fn invalid_params(id: Value, details: &str) -> Self {
        Self::error(id, -32602, "Invalid params", Some(json!({ "details": details })))
    }

    pub fn parse_error(id: Value, details: &str) -> Self {
        Self::error(id, -32700, "Parse error", Some(json!({ "details": details })))
    }
}

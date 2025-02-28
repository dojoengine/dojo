use std::net::IpAddr;
use std::sync::Arc;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

use futures_util::{SinkExt, StreamExt};
use hyper::{Body, Method, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use tokio_tungstenite::tungstenite::Message;

use super::sql::map_row_to_json;
use super::Handler;

const JSONRPC_VERSION: &str = "2.0";
const MCP_VERSION: &str = "2024-11-05";

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Value,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcNotification {
    jsonrpc: String,
    method: String,
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

#[derive(Debug, Serialize)]
struct Implementation {
    name: String,
    version: String,
}

#[derive(Debug, Serialize)]
struct ServerCapabilities {
    tools: ToolCapabilities,
    resources: ResourceCapabilities,
}

#[derive(Debug, Serialize)]
struct ToolCapabilities {
    list_changed: bool,
}

#[derive(Debug, Serialize)]
struct ResourceCapabilities {
    subscribe: bool,
    list_changed: bool,
}

#[derive(Debug, Clone)]
pub struct McpHandler {
    pool: Arc<SqlitePool>,
    // Store active SSE connections by session ID
    active_sessions: Arc<Mutex<HashMap<String, bool>>>,
}

impl McpHandler {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self { 
            pool,
            active_sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn generate_session_id(&self) -> String {
        let session_id = Uuid::new_v4().to_string();
        // Register the session
        let mut sessions = self.active_sessions.lock().unwrap();
        sessions.insert(session_id.clone(), true);
        session_id
    }

    fn is_active_session(&self, id: &str) -> bool {
        let sessions = self.active_sessions.lock().unwrap();
        sessions.contains_key(id)
    }

    async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        if request.jsonrpc != JSONRPC_VERSION {
            return JsonRpcResponse::invalid_request(request.id);
        }

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "tools/list" => self.handle_tools_list(request.id),
            "tools/call" => self.handle_tools_call(request).await,
            "resources/list" => self.handle_resources_list(request.id).await,
            "resources/read" => self.handle_resources_read(request).await,
            "resources/subscribe" => self.handle_resources_subscribe(request).await,
            "resources/unsubscribe" => self.handle_resources_unsubscribe(request).await,
            "ping" => JsonRpcResponse::ok(request.id, json!({})),
            _ => JsonRpcResponse::method_not_found(request.id),
        }
    }

    fn handle_initialize(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse::ok(
            id,
            json!({
                "protocolVersion": MCP_VERSION,
                "serverInfo": Implementation {
                    name: "torii-mcp".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                },
                "capabilities": ServerCapabilities {
                    tools: ToolCapabilities {
                        list_changed: true,
                    },
                    resources: ResourceCapabilities {
                        subscribe: true,
                        list_changed: true,
                    },
                },
                "instructions": include_str!("../../static/mcp-instructions.txt")
            }),
        )
    }

    fn handle_tools_list(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse::ok(
            id,
            json!({
                "tools": [
                    {
                        "name": "query",
                        "description": "Execute a SQL query on the database",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "query": {
                                    "type": "string",
                                    "description": "SQL query to execute"
                                }
                            },
                            "required": ["query"]
                        }
                    },
                    {
                        "name": "schema",
                        "description": "Retrieve the database schema including tables, columns, and their types",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "table": {
                                    "type": "string",
                                    "description": "Optional table name to get schema for. If omitted, returns schema for all tables."
                                }
                            }
                        }
                    }
                ]
            }),
        )
    }

    async fn handle_tools_call(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let Some(params) = &request.params else {
            return JsonRpcResponse::invalid_params(request.id, "Missing params");
        };

        let Some(tool_name) = params.get("name").and_then(Value::as_str) else {
            return JsonRpcResponse::invalid_params(request.id, "Missing tool name");
        };

        match tool_name {
            "query" => self.handle_query_tool(request).await,
            "schema" => self.handle_schema_tool(request).await,
            _ => JsonRpcResponse::method_not_found(request.id),
        }
    }

    async fn handle_notification(&self, notification: JsonRpcNotification) {
        match notification.method.as_str() {
            "notifications/initialized" => {
                // Handle initialized notification if needed
            }
            "notifications/cancelled" => {
                // Handle cancellation notification if needed
            }
            _ => {
                // Handle other notifications if needed
            }
        }
    }

    async fn handle_resources_list(&self, id: Value) -> JsonRpcResponse {
        // For now, return an empty list
        JsonRpcResponse::ok(
            id,
            json!({
                "resources": []
            }),
        )
    }

    async fn handle_resources_read(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let Some(params) = &request.params else {
            return JsonRpcResponse::invalid_params(request.id, "Missing params");
        };

        let Some(uri) = params.get("uri").and_then(Value::as_str) else {
            return JsonRpcResponse::invalid_params(request.id, "Missing uri parameter");
        };

        // For now, return a simple error since we don't have actual resource handling
        JsonRpcResponse::error(
            request.id,
            -32602,
            "Resource not found",
            Some(json!({ "details": format!("Resource '{}' not found", uri) })),
        )
    }

    async fn handle_resources_subscribe(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let Some(params) = &request.params else {
            return JsonRpcResponse::invalid_params(request.id, "Missing params");
        };

        let Some(uri) = params.get("uri").and_then(Value::as_str) else {
            return JsonRpcResponse::invalid_params(request.id, "Missing uri parameter");
        };

        // For now, just acknowledge the subscription
        JsonRpcResponse::ok(
            request.id,
            json!({}),
        )
    }

    async fn handle_resources_unsubscribe(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let Some(params) = &request.params else {
            return JsonRpcResponse::invalid_params(request.id, "Missing params");
        };

        let Some(uri) = params.get("uri").and_then(Value::as_str) else {
            return JsonRpcResponse::invalid_params(request.id, "Missing uri parameter");
        };

        // For now, just acknowledge the unsubscription
        JsonRpcResponse::ok(
            request.id,
            json!({}),
        )
    }

    async fn handle_websocket_connection(
        &self,
        ws_stream: tokio_tungstenite::WebSocketStream<hyper::upgrade::Upgraded>,
    ) {
        let (mut write, mut read) = ws_stream.split();
        let session_id = self.generate_session_id();
        
        if let Err(e) = write.send(Message::Text(format!("{{\"type\":\"connection_id\",\"id\":\"{}\"}}", session_id))).await {
            eprintln!("Error sending session ID: {}", e);
            return;
        }

        while let Some(msg) = read.next().await {
            if let Ok(Message::Text(text)) = msg {
                let response = match serde_json::from_str::<JsonRpcMessage>(&text) {
                    Ok(JsonRpcMessage::Request(request)) => self.handle_request(request).await,
                    Ok(JsonRpcMessage::Notification(notification)) => {
                        self.handle_notification(notification).await;
                        continue;
                    }
                    Err(e) => JsonRpcResponse::parse_error(Value::Null, &e.to_string()),
                };

                if let Err(e) =
                    write.send(Message::Text(serde_json::to_string(&response).unwrap())).await
                {
                    eprintln!("Error sending message: {}", e);
                    break;
                }
            }
        }
        
        // Clean up session when connection closes
        let mut sessions = self.active_sessions.lock().unwrap();
        sessions.remove(&session_id);
    }

    // Handle initial SSE connection setup
    async fn handle_sse_setup(&self) -> Response<Body> {
        let session_id = self.generate_session_id();
        let message_url = format!("/mcp/message?sessionId={}", session_id);
        
        // Return SSE format with endpoint event
        let sse_response = format!("event: endpoint\ndata: {}\n\n", message_url);
        
        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(Body::from(sse_response))
            .unwrap()
    }

    // Handle JSON-RPC requests to the message endpoint
    async fn handle_message_request(&self, req: Request<Body>) -> Response<Body> {
        // Extract session ID from query parameters
        let session_id = req.uri().query().and_then(|q| {
            q.split('&')
                .find_map(|p| {
                    let parts: Vec<&str> = p.split('=').collect();
                    if parts.len() == 2 && parts[0] == "sessionId" {
                        Some(parts[1])
                    } else {
                        None
                    }
                })
        });
        
        // Verify this is a valid session
        if session_id.is_none() || !self.is_active_session(session_id.unwrap()) {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("Invalid or missing session ID"))
                .unwrap();
        }
        
        // Parse the request body
        let body_bytes = hyper::body::to_bytes(req.into_body()).await.unwrap_or_default();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap_or_default();
        
        // Process the JSON-RPC request
        let response = match serde_json::from_str::<JsonRpcMessage>(&body_str) {
            Ok(JsonRpcMessage::Request(request)) => {
                let resp = self.handle_request(request).await;
                serde_json::to_string(&resp).unwrap_or_default()
            },
            Ok(JsonRpcMessage::Notification(_)) => {
                // Notifications don't expect responses
                return Response::builder()
                    .status(StatusCode::NO_CONTENT)
                    .body(Body::empty())
                    .unwrap();
            },
            Err(e) => {
                let error_resp = JsonRpcResponse::parse_error(Value::Null, &e.to_string());
                serde_json::to_string(&error_resp).unwrap_or_default()
            },
        };
        
        // Return the JSON-RPC response
        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Body::from(response))
            .unwrap()
    }

    async fn handle_schema_tool(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let table_filter = request
            .params
            .as_ref()
            .and_then(|p| p.get("arguments"))
            .and_then(|args| args.get("table"))
            .and_then(Value::as_str);

        let schema_query = match table_filter {
            Some(_table) => "SELECT 
                    m.name as table_name,
                    p.* 
                FROM sqlite_master m
                JOIN pragma_table_info(m.name) p
                WHERE m.type = 'table'
                AND m.name = ?
                ORDER BY m.name, p.cid"
                .to_string(),
            _ => "SELECT 
                    m.name as table_name,
                    p.* 
                FROM sqlite_master m
                JOIN pragma_table_info(m.name) p
                WHERE m.type = 'table'
                ORDER BY m.name, p.cid"
                .to_string(),
        };

        let rows = match table_filter {
            Some(table) => sqlx::query(&schema_query).bind(table).fetch_all(&*self.pool).await,
            _ => sqlx::query(&schema_query).fetch_all(&*self.pool).await,
        };

        match rows {
            Ok(rows) => {
                let mut schema = serde_json::Map::new();

                for row in rows {
                    let table_name: String = row.try_get("table_name").unwrap();
                    let column_name: String = row.try_get("name").unwrap();
                    let column_type: String = row.try_get("type").unwrap();
                    let not_null: bool = row.try_get::<bool, _>("notnull").unwrap();
                    let pk: bool = row.try_get::<bool, _>("pk").unwrap();
                    let default_value: Option<String> = row.try_get("dflt_value").unwrap();

                    let table_entry = schema.entry(table_name).or_insert_with(|| {
                        json!({
                            "columns": serde_json::Map::new()
                        })
                    });

                    if let Some(columns) =
                        table_entry.get_mut("columns").and_then(|v| v.as_object_mut())
                    {
                        columns.insert(
                            column_name,
                            json!({
                                "type": column_type,
                                "nullable": !not_null,
                                "primary_key": pk,
                                "default": default_value
                            }),
                        );
                    }
                }

                JsonRpcResponse {
                    jsonrpc: JSONRPC_VERSION.to_string(),
                    id: request.id,
                    result: Some(json!({
                        "content": [{
                            "type": "text",
                            "text": serde_json::to_string_pretty(&schema).unwrap()
                        }]
                    })),
                    error: None,
                }
            }
            Err(e) => JsonRpcResponse {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32603,
                    message: "Database error".to_string(),
                    data: Some(json!({ "details": e.to_string() })),
                }),
            },
        }
    }

    async fn handle_query_tool(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        if let Some(params) = request.params {
            if let Some(query) = params.get("arguments").and_then(Value::as_str) {
                match sqlx::query(query).fetch_all(&*self.pool).await {
                    Ok(rows) => {
                        // Convert rows to JSON using shared mapping function
                        let result = rows.iter().map(map_row_to_json).collect::<Vec<_>>();

                        JsonRpcResponse {
                            jsonrpc: JSONRPC_VERSION.to_string(),
                            id: request.id,
                            result: Some(json!({
                                "content": [{
                                    "type": "text",
                                    "text": serde_json::to_string(&result).unwrap()
                                }]
                            })),
                            error: None,
                        }
                    }
                    Err(e) => JsonRpcResponse {
                        jsonrpc: JSONRPC_VERSION.to_string(),
                        id: request.id,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32603,
                            message: "Database error".to_string(),
                            data: Some(json!({ "details": e.to_string() })),
                        }),
                    },
                }
            } else {
                JsonRpcResponse {
                    jsonrpc: JSONRPC_VERSION.to_string(),
                    id: request.id,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32602,
                        message: "Invalid params".to_string(),
                        data: Some(json!({ "details": "Missing query parameter" })),
                    }),
                }
            }
        } else {
            JsonRpcResponse {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32602,
                    message: "Invalid params".to_string(),
                    data: None,
                }),
            }
        }
    }
}

impl JsonRpcResponse {
    fn ok(id: Value, result: Value) -> Self {
        Self { jsonrpc: JSONRPC_VERSION.to_string(), id, result: Some(result), error: None }
    }

    fn error(id: Value, code: i32, message: &str, data: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(JsonRpcError { code, message: message.to_string(), data }),
        }
    }

    fn invalid_request(id: Value) -> Self {
        Self::error(id, -32600, "Invalid Request", None)
    }

    fn method_not_found(id: Value) -> Self {
        Self::error(id, -32601, "Method not found", None)
    }

    fn invalid_params(id: Value, details: &str) -> Self {
        Self::error(id, -32602, "Invalid params", Some(json!({ "details": details })))
    }

    fn parse_error(id: Value, details: &str) -> Self {
        Self::error(id, -32700, "Parse error", Some(json!({ "details": details })))
    }
}

#[async_trait::async_trait]
impl Handler for McpHandler {
    fn should_handle(&self, req: &Request<Body>) -> bool {
        req.uri().path().starts_with("/mcp")
    }

    async fn handle(&self, req: Request<Body>) -> Response<Body> {
        // Handle WebSocket upgrade requests
        if req.headers()
            .get("upgrade")
            .and_then(|h| h.to_str().ok())
            .map(|h| h.eq_ignore_ascii_case("websocket"))
            .unwrap_or(false) 
        {
            if hyper_tungstenite::is_upgrade_request(&req) {
                let (response, websocket) = hyper_tungstenite::upgrade(req, None)
                    .expect("Failed to upgrade WebSocket connection");

                let this = self.clone();
                tokio::spawn(async move {
                    if let Ok(ws_stream) = websocket.await {
                        this.handle_websocket_connection(ws_stream).await;
                    }
                });

                return response;
            } else {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from("Invalid WebSocket upgrade request"))
                    .unwrap();
            }
        }
        
        // Handle HTTP requests
        match (req.method(), req.uri().path()) {
            // GET /mcp - Set up SSE connection
            (&Method::GET, "/mcp") => {
                self.handle_sse_setup().await
            },
            
            // POST /mcp/message - Handle JSON-RPC requests
            (&Method::POST, path) if path.starts_with("/mcp/message") => {
                self.handle_message_request(req).await
            },
            
            // Other methods not allowed
            _ => {
                Response::builder()
                    .status(StatusCode::METHOD_NOT_ALLOWED)
                    .body(Body::from("Method not allowed"))
                    .unwrap()
            }
        }
    }
}

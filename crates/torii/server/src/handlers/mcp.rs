use std::net::IpAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use hyper::{Body, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Number, Value};
use sqlx::{Row, SqlitePool};
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::tungstenite::Message;
use tracing::warn;
use uuid::Uuid;

use super::sql::map_row_to_json;
use super::Handler;

const JSONRPC_VERSION: &str = "2.0";
const MCP_VERSION: &str = "2024-11-05";
const SSE_CHANNEL_CAPACITY: usize = 100;

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
    _jsonrpc: String,
    _method: String,
    _params: Option<Value>,
}

#[derive(Debug, Serialize, Clone)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Clone)]
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

// Structure to hold SSE session information
#[derive(Clone, Debug)]
struct SseSession {
    tx: broadcast::Sender<JsonRpcResponse>,
    _session_id: String,
}

#[derive(Clone, Debug)]
struct Tool {
    name: &'static str,
    description: &'static str,
    input_schema: Value,
}

#[derive(Clone, Debug)]
struct Resource {
    name: &'static str,
}

#[derive(Clone, Debug)]
pub struct McpHandler {
    pool: Arc<SqlitePool>,
    sse_sessions: Arc<RwLock<std::collections::HashMap<String, SseSession>>>,
    tools: Vec<Tool>,
    resources: Vec<Resource>,
}

impl McpHandler {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        let tools = vec![
            Tool {
                name: "query",
                description: "Execute a SQL query on the database",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "SQL query to execute"
                        }
                    },
                    "required": ["query"]
                }),
            },
            Tool {
                name: "schema",
                description: "Retrieve the database schema including tables, columns, and their \
                              types",
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "table": {
                            "type": "string",
                            "description": "Optional table name to get schema for. If omitted, returns schema for all tables."
                        }
                    }
                }),
            },
        ];

        let resources = vec![]; // Add resources as needed

        Self {
            pool,
            sse_sessions: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            tools,
            resources,
        }
    }

    async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        if request.jsonrpc != JSONRPC_VERSION {
            return JsonRpcResponse::invalid_request(request.id);
        }

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "tools/list" => self.handle_tools_list(request.id),
            "tools/call" => self.handle_tools_call(request).await,
            "resources/list" => self.handle_resources_list(request.id),
            "resources/read" => self.handle_resources_read(request).await,
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
        let tools_json: Vec<Value> = self
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "inputSchema": tool.input_schema
                })
            })
            .collect();

        JsonRpcResponse::ok(id, json!({ "tools": tools_json }))
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

    async fn handle_websocket_connection(
        &self,
        ws_stream: tokio_tungstenite::WebSocketStream<hyper::upgrade::Upgraded>,
    ) {
        let (mut write, mut read) = ws_stream.split();

        while let Some(msg) = read.next().await {
            if let Ok(Message::Text(text)) = msg {
                let response = match serde_json::from_str::<JsonRpcMessage>(&text) {
                    Ok(JsonRpcMessage::Request(request)) => self.handle_request(request).await,
                    Ok(JsonRpcMessage::Notification(_notification)) => {
                        // Handle notifications if needed
                        continue;
                    }
                    Err(e) => JsonRpcResponse::parse_error(Value::Null, &e.to_string()),
                };

                if let Err(e) =
                    write.send(Message::Text(serde_json::to_string(&response).unwrap())).await
                {
                    warn!("Error sending message: {}", e);
                    break;
                }
            }
        }
    }

    // New method to handle SSE connections
    async fn handle_sse_connection(&self) -> Response<Body> {
        // Create a new session ID
        let session_id = Uuid::new_v4().to_string();

        // Create a broadcast channel for SSE messages
        let (tx, rx) = broadcast::channel::<JsonRpcResponse>(SSE_CHANNEL_CAPACITY);

        // Store the session
        {
            let mut sessions = self.sse_sessions.write().await;
            sessions.insert(
                session_id.clone(),
                SseSession { tx: tx.clone(), _session_id: session_id.clone() },
            );
        }

        // Create the message endpoint path
        let message_endpoint = format!("/mcp/message?sessionId={}", session_id);

        // Create initial endpoint info event - using full URL format
        let endpoint_info = format!("event: endpoint\ndata: {}\n\n", message_endpoint);

        // Create the streaming body with the endpoint information followed by event data
        let stream = futures_util::stream::once(async move {
            Ok::<_, hyper::Error>(hyper::body::Bytes::from(endpoint_info))
        })
        .chain(futures_util::stream::unfold(rx, move |mut rx| {
            async move {
                match rx.recv().await {
                    Ok(msg) => {
                        match serde_json::to_string(&msg) {
                            Ok(json) => {
                                // Format SSE data with event name and proper line breaks
                                let sse_data = format!("event: message\ndata: {}\n\n", json);
                                Some((
                                    Ok::<_, hyper::Error>(hyper::body::Bytes::from(sse_data)),
                                    rx,
                                ))
                            }
                            Err(e) => {
                                warn!("Error serializing message: {}", e);
                                // Format error event with proper SSE format
                                Some((
                                    Ok::<_, hyper::Error>(hyper::body::Bytes::from(format!(
                                        "event: error\ndata: {{\n  \"error\": \"{}\" }}\n\n",
                                        e
                                    ))),
                                    rx,
                                ))
                            }
                        }
                    }
                    Err(_) => None,
                }
            }
        }));

        // Return the SSE response
        Response::builder()
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .header("X-Session-Id", session_id)
            .body(Body::wrap_stream(stream))
            .unwrap()
    }

    // New method to handle JSON-RPC messages sent via HTTP POST
    async fn handle_message_request(&self, req: Request<Body>) -> Response<Body> {
        // Extract the session ID from the query parameters
        let uri = req.uri();
        let session_id = uri.query().unwrap().split("=").collect::<Vec<_>>()[1];

        // Check if the session exists
        let tx = {
            let sessions = self.sse_sessions.read().await;
            match sessions.get(session_id) {
                Some(s) => s.tx.clone(),
                _ => {
                    return Response::builder()
                        .body(Body::from(
                            serde_json::to_string(&JsonRpcResponse::invalid_params(
                                Value::Number(Into::<Number>::into(-1)),
                                "session not found",
                            ))
                            .unwrap(),
                        ))
                        .unwrap();
                }
            }
        };

        // Read the request body
        let body_bytes = match hyper::body::to_bytes(req.into_body()).await {
            Ok(bytes) => bytes,
            Err(e) => {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(format!("Error reading request body: {}", e)))
                    .unwrap();
            }
        };

        let body_str = match String::from_utf8(body_bytes.to_vec()) {
            Ok(s) => s,
            Err(e) => {
                return Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(format!("Invalid UTF-8 in request body: {}", e)))
                    .unwrap();
            }
        };

        // First try to parse as a raw JSON value to handle any valid JSON input
        let parsed_json: Result<serde_json::Value, _> = serde_json::from_str(&body_str);

        let response = match &parsed_json {
            Ok(json_value) => {
                // Try to parse as a JsonRpcMessage
                match serde_json::from_value::<JsonRpcMessage>(json_value.clone()) {
                    Ok(JsonRpcMessage::Request(request)) => {
                        let response = self.handle_request(request).await;

                        // Forward the response to the SSE channel
                        if let Err(e) = tx.send(response.clone()) {
                            warn!("Error forwarding response to SSE: {}", e);
                        }

                        Response::builder()
                            .status(StatusCode::ACCEPTED)
                            .header("Content-Type", "application/json")
                            .header("Access-Control-Allow-Origin", "*")
                            .body(Body::from(serde_json::to_string(&response).unwrap()))
                            .unwrap()
                    }
                    Ok(JsonRpcMessage::Notification(_)) => {
                        // For notifications, just send 202 Accepted with no body
                        Response::builder()
                            .status(StatusCode::ACCEPTED)
                            .header("Access-Control-Allow-Origin", "*")
                            .body(Body::empty())
                            .unwrap()
                    }
                    Err(_) => {
                        // If not a valid JsonRpcMessage, try to interpret as a raw request
                        // This is more permissive and handles cases where the client sends
                        // simplified JSON
                        if let Some(method) = json_value.get("method").and_then(|m| m.as_str()) {
                            let id = json_value.get("id").cloned().unwrap_or(Value::Null);
                            let params = json_value.get("params").cloned();

                            let request = JsonRpcRequest {
                                jsonrpc: JSONRPC_VERSION.to_string(),
                                id,
                                method: method.to_string(),
                                params,
                            };

                            let response = self.handle_request(request).await;

                            // Forward the response to the SSE channel
                            if let Err(e) = tx.send(response.clone()) {
                                warn!("Error forwarding response to SSE: {}", e);
                            }

                            Response::builder()
                                .status(StatusCode::ACCEPTED)
                                .header("Content-Type", "application/json")
                                .header("Access-Control-Allow-Origin", "*")
                                .body(Body::from(serde_json::to_string(&response).unwrap()))
                                .unwrap()
                        } else {
                            // Not a valid request
                            let error_response = JsonRpcResponse::invalid_request(Value::Null);
                            Response::builder()
                                .status(StatusCode::BAD_REQUEST)
                                .header("Content-Type", "application/json")
                                .header("Access-Control-Allow-Origin", "*")
                                .body(Body::from(serde_json::to_string(&error_response).unwrap()))
                                .unwrap()
                        }
                    }
                }
            }
            Err(e) => {
                let error_response = JsonRpcResponse::parse_error(Value::Null, &e.to_string());
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .header("Content-Type", "application/json")
                    .header("Access-Control-Allow-Origin", "*")
                    .body(Body::from(serde_json::to_string(&error_response).unwrap()))
                    .unwrap()
            }
        };

        response
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
        let Some(params) = request.params else {
            return JsonRpcResponse::invalid_params(request.id, "Missing params");
        };

        let args = params.get("arguments").and_then(Value::as_object);
        if let Some(query) = args.and_then(|args| args.get("query").and_then(Value::as_str)) {
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
    }

    // New method to handle resources/list
    fn handle_resources_list(&self, id: Value) -> JsonRpcResponse {
        let resources_json: Vec<Value> =
            self.resources.iter().map(|resource| json!({ "name": resource.name })).collect();

        JsonRpcResponse::ok(id, json!({ "resources": resources_json }))
    }

    // New method to handle resources/read
    async fn handle_resources_read(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let Some(params) = &request.params else {
            return JsonRpcResponse::invalid_params(request.id, "Missing params");
        };

        let Some(uri) = params.get("uri").and_then(Value::as_str) else {
            return JsonRpcResponse::invalid_params(request.id, "Missing uri parameter");
        };

        // For now, we don't have any resources to read
        JsonRpcResponse {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: request.id,
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: "Resource not found".to_string(),
                data: Some(json!({ "details": format!("No resource found with URI: {}", uri) })),
            }),
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

    async fn handle(&self, req: Request<Body>, _: IpAddr) -> Response<Body> {
        let uri_path = req.uri().path();

        // Handle CORS preflight requests
        if req.method() == hyper::Method::OPTIONS {
            return Response::builder()
                .status(StatusCode::OK)
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type, Authorization")
                .header("Access-Control-Max-Age", "86400")
                .body(Body::empty())
                .unwrap();
        }

        // Handle message endpoint (for SSE clients)
        if uri_path == "/mcp/message" {
            return self.handle_message_request(req).await;
        }

        match req.method() {
            // Handle GET requests for SSE connection
            &hyper::Method::GET => {
                return self.handle_sse_connection().await;
            }
            // Handle WebSocket upgrade requests
            _ if hyper_tungstenite::is_upgrade_request(&req) => {
                let (response, websocket) = match hyper_tungstenite::upgrade(req, None) {
                    Ok(upgrade) => upgrade,
                    Err(_) => {
                        return Response::builder()
                            .status(StatusCode::BAD_REQUEST)
                            .header("Access-Control-Allow-Origin", "*")
                            .body(Body::from("Failed to upgrade WebSocket connection"))
                            .unwrap();
                    }
                };

                let this = self.clone();
                tokio::spawn(async move {
                    if let Ok(ws_stream) = websocket.await {
                        this.handle_websocket_connection(ws_stream).await;
                    }
                });

                response
            }
            // Return Method Not Allowed for other methods
            _ => Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .header("Access-Control-Allow-Origin", "*")
                .body(Body::from("Method not allowed"))
                .unwrap(),
        }
    }
}

use std::sync::Arc;
use futures_util::{SinkExt, StreamExt};
use hyper::{Body, Request, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{SqlitePool, Row};
use tokio_tungstenite::tungstenite::Message;

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

pub struct McpHandler {
    pool: Arc<SqlitePool>,
}

impl McpHandler {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }

    async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => {
                JsonRpcResponse {
                    jsonrpc: JSONRPC_VERSION.to_string(),
                    id: request.id,
                    result: Some(json!({
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
                        }
                    })),
                    error: None,
                }
            },
            "tools/list" => {
                JsonRpcResponse {
                    jsonrpc: JSONRPC_VERSION.to_string(),
                    id: request.id,
                    result: Some(json!({
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
                    })),
                    error: None,
                }
            },
            "tools/call" => {
                if let Some(params) = &request.params {
                    match params.get("name").and_then(Value::as_str) {
                        Some("query") => self.handle_query_tool(request).await,
                        Some("schema") => self.handle_schema_tool(request).await,
                        _ => JsonRpcResponse {
                            jsonrpc: JSONRPC_VERSION.to_string(),
                            id: request.id,
                            result: None,
                            error: Some(JsonRpcError {
                                code: -32601,
                                message: "Tool not found".to_string(),
                                data: None,
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
                            data: None,
                        }),
                    }
                }
            },
            _ => JsonRpcResponse {
                jsonrpc: JSONRPC_VERSION.to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: -32601,
                    message: "Method not found".to_string(),
                    data: None,
                }),
            },
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
                    Ok(JsonRpcMessage::Request(request)) => {
                        self.handle_request(request).await
                    },
                    Ok(JsonRpcMessage::Notification(_notification)) => {
                        // Handle notifications if needed
                        continue;
                    },
                    Err(e) => JsonRpcResponse {
                        jsonrpc: JSONRPC_VERSION.to_string(),
                        id: Value::Null,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32700,
                            message: "Parse error".to_string(),
                            data: Some(json!({ "details": e.to_string() })),
                        }),
                    },
                };

                if let Err(e) = write
                    .send(Message::Text(serde_json::to_string(&response).unwrap()))
                    .await
                {
                    eprintln!("Error sending message: {}", e);
                    break;
                }
            }
        }
    }

    async fn handle_schema_tool(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let table_filter = request.params
            .as_ref()
            .and_then(|p| p.get("arguments"))
            .and_then(|args| args.get("table"))
            .and_then(Value::as_str);

        let schema_query = match table_filter {
            Some(table) => format!(
                "SELECT 
                    m.name as table_name,
                    p.* 
                FROM sqlite_master m
                JOIN pragma_table_info(m.name) p
                WHERE m.type = 'table'
                AND m.name = ?
                ORDER BY m.name, p.cid"
            ),
            None => format!(
                "SELECT 
                    m.name as table_name,
                    p.* 
                FROM sqlite_master m
                JOIN pragma_table_info(m.name) p
                WHERE m.type = 'table'
                ORDER BY m.name, p.cid"
            ),
        };

        let rows = match table_filter {
            Some(table) => sqlx::query(&schema_query)
                .bind(table)
                .fetch_all(&*self.pool)
                .await,
            None => sqlx::query(&schema_query)
                .fetch_all(&*self.pool)
                .await,
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

                    let table_entry = schema.entry(table_name)
                        .or_insert_with(|| json!({
                            "columns": serde_json::Map::new()
                        }));

                    if let Some(columns) = table_entry.get_mut("columns").and_then(|v| v.as_object_mut()) {
                        columns.insert(column_name, json!({
                            "type": column_type,
                            "nullable": !not_null,
                            "primary_key": pk,
                            "default": default_value
                        }));
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
            },
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
            if let Some(query) = params.get("query").and_then(Value::as_str) {
                match sqlx::query(query).fetch_all(&*self.pool).await {
                    Ok(rows) => {
                        // Convert rows to JSON using the same logic as SqlHandler
                        let result = rows.iter().map(|row| {
                            let mut obj = serde_json::Map::new();
                            for (i, column) in row.columns().iter().enumerate() {
                                // ... row conversion logic from SqlHandler ...
                            }
                            Value::Object(obj)
                        }).collect::<Vec<_>>();

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
                    },
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

#[async_trait::async_trait]
impl Handler for McpHandler {
    fn should_handle(&self, req: &Request<Body>) -> bool {
        req.uri().path().starts_with("/mcp") && req.headers()
            .get("upgrade")
            .and_then(|h| h.to_str().ok())
            .map(|h| h.eq_ignore_ascii_case("websocket"))
            .unwrap_or(false)
    }

    async fn handle(&self, req: Request<Body>) -> Response<Body> {
        if hyper_tungstenite::is_upgrade_request(&req) {
            let (response, websocket) = hyper_tungstenite::upgrade(req, None)
                .expect("Failed to upgrade WebSocket connection");

            let this = self.clone();
            tokio::spawn(async move {
                if let Ok(ws_stream) = websocket.await {
                    this.handle_websocket_connection(ws_stream).await;
                }
            });

            response
        } else {
            Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("Not a WebSocket upgrade request"))
                .unwrap()
        }
    }
} 
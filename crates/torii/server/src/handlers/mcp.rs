use std::net::IpAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use hyper::{Body, Request, Response, StatusCode};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use tokio::sync::{broadcast, RwLock};
use tokio_tungstenite::tungstenite::Message;
use torii_mcp::{tools::{self, Tool}, resources::{self, Resource}};
use tracing::warn;
use uuid::Uuid;

use torii_mcp::types::{
    JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, SseSession,
    JSONRPC_VERSION, MCP_VERSION, SSE_CHANNEL_CAPACITY,
};

use super::Handler;

#[derive(Clone, Debug)]
pub struct McpHandler {
    pool: Arc<SqlitePool>,
    sse_sessions: Arc<RwLock<std::collections::HashMap<String, SseSession>>>,
    tools: Vec<Tool>,
    resources: Vec<Resource>,
}

impl McpHandler {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self {
            pool,
            sse_sessions: Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
            tools: tools::get_tools(),
            resources: resources::get_resources(),
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
                "serverInfo": {
                    "name": "torii-mcp",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": {
                    "tools": {
                        "list_changed": true,
                    },
                    "resources": {
                        "subscribe": true,
                        "list_changed": true,
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
            "query" => tools::query::handle(self.pool.clone(), request).await,
            "schema" => tools::schema::handle(self.pool.clone(), request).await,
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

    // Method to handle SSE connections
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
                    Err(e) => {
                        warn!("Error receiving message: {}", e);
                        // Return error and continue
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
        }));

        // Build the response with appropriate headers for SSE
        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(Body::wrap_stream(stream))
            .unwrap()
    }

    async fn handle_message_request(&self, req: Request<Body>) -> Response<Body> {
        // Extract session ID from query parameters
        let session_id = req
            .uri()
            .query()
            .and_then(|q| {
                q.split('&')
                    .find_map(|p| {
                        let parts: Vec<&str> = p.split('=').collect();
                        if parts.len() == 2 && parts[0] == "sessionId" {
                            Some(parts[1].to_string())
                        } else {
                            None
                        }
                    })
            });

        if session_id.is_none() {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("Missing sessionId parameter"))
                .unwrap();
        }

        let session_id = session_id.unwrap();

        // Check if the session exists
        let tx = {
            let sessions = self.sse_sessions.read().await;
            sessions.get(&session_id).map(|s| s.tx.clone())
        };

        if tx.is_none() {
            return Response::builder()
                .status(StatusCode::NOT_FOUND)
                .body(Body::from("Session not found"))
                .unwrap();
        }

        let tx = tx.unwrap();

        // Read the request body
        let body_bytes = hyper::body::to_bytes(req.into_body()).await.unwrap();
        let body_str = String::from_utf8(body_bytes.to_vec()).unwrap();

        // Parse the JSON-RPC request
        let response = match serde_json::from_str::<JsonRpcMessage>(&body_str) {
            Ok(JsonRpcMessage::Request(request)) => {
                let response = self.handle_request(request).await;
                // Send the response to the SSE channel
                if let Err(e) = tx.send(response.clone()) {
                    warn!("Error sending message to SSE channel: {}", e);
                }
                response
            }
            Ok(JsonRpcMessage::Notification(_notification)) => {
                // Handle notifications if needed
                JsonRpcResponse::ok(Value::Null, json!({}))
            }
            Err(e) => JsonRpcResponse::parse_error(Value::Null, &e.to_string()),
        };

        // Return the response
        Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&response).unwrap()))
            .unwrap()
    }

    fn handle_resources_list(&self, id: Value) -> JsonRpcResponse {
        let resources_json: Vec<Value> =
            self.resources.iter().map(|resource| json!({ "name": resource.name })).collect();

        JsonRpcResponse::ok(id, json!({ "resources": resources_json }))
    }

    async fn handle_resources_read(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let Some(params) = &request.params else {
            return JsonRpcResponse::invalid_params(request.id, "Missing params");
        };

        let Some(_resource_name) = params.get("name").and_then(Value::as_str) else {
            return JsonRpcResponse::invalid_params(request.id, "Missing resource name");
        };

        // Implement resource reading logic here
        // For now, return method not found
        JsonRpcResponse::method_not_found(request.id)
    }
}

#[async_trait::async_trait]
impl Handler for McpHandler {
    fn should_handle(&self, req: &Request<Body>) -> bool {
        req.uri().path().starts_with("/mcp")
    }

    async fn handle(&self, req: Request<Body>, _: IpAddr) -> Response<Body> {
        // Handle WebSocket upgrade requests
        if hyper_tungstenite::is_upgrade_request(&req) {
            let (response, websocket) = hyper_tungstenite::upgrade(req, None).unwrap();
            let self_clone = self.clone();

            // Spawn a task to handle the WebSocket connection
            tokio::spawn(async move {
                if let Ok(ws_stream) = websocket.await {
                    self_clone.handle_websocket_connection(ws_stream).await;
                }
            });

            return response;
        }

        // Handle message requests for SSE
        if req.uri().path() == "/mcp/message" {
            return self.handle_message_request(req).await;
        }

        match req.method() {
            // Handle GET requests for SSE connection
            &hyper::Method::GET => {
                return self.handle_sse_connection().await;
            }
            // Return Method Not Allowed for other methods
            _ => Response::builder()
                .body(Body::from(
                    serde_json::to_string(&JsonRpcResponse::method_not_found(Value::Null)).unwrap(),
                ))
                .unwrap(),
        }
    }
} 
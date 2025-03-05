use std::sync::Arc;

use serde_json::{json, Value};
use sqlx::SqlitePool;
use torii_sqlite::utils::map_row_to_json;

use super::Tool;
use crate::types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, JSONRPC_VERSION};

pub fn get_tool() -> Tool {
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
    }
}

pub async fn handle(pool: Arc<SqlitePool>, request: JsonRpcRequest) -> JsonRpcResponse {
    let Some(params) = request.params else {
        return JsonRpcResponse::invalid_params(request.id, "Missing params");
    };

    let args = params.get("arguments").and_then(Value::as_object);
    if let Some(query) = args.and_then(|args| args.get("query").and_then(Value::as_str)) {
        match sqlx::query(query).fetch_all(&*pool).await {
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

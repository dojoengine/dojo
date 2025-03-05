use std::sync::Arc;

use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};

use super::Tool;
use crate::types::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, JSONRPC_VERSION};

pub fn get_tool() -> Tool {
    Tool {
        name: "schema",
        description: "Retrieve the database schema including tables, columns, and their types",
        input_schema: json!({
            "type": "object",
            "properties": {
                "table": {
                    "type": "string",
                    "description": "Optional table name to get schema for. If omitted, returns schema for all tables."
                }
            }
        }),
    }
}

pub async fn handle(pool: Arc<SqlitePool>, request: JsonRpcRequest) -> JsonRpcResponse {
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
        Some(table) => sqlx::query(&schema_query).bind(table).fetch_all(&*pool).await,
        _ => sqlx::query(&schema_query).fetch_all(&*pool).await,
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

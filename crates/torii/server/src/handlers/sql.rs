use std::sync::Arc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use http::header::CONTENT_TYPE;
use hyper::{Body, Method, Request, Response, StatusCode};
use sqlx::{Column, Row, SqlitePool, TypeInfo};

use super::Handler;

pub struct SqlHandler {
    pool: Arc<SqlitePool>,
}

impl SqlHandler {
    pub fn new(pool: Arc<SqlitePool>) -> Self {
        Self { pool }
    }

    pub async fn execute_query(&self, query: String) -> Response<Body> {
        match sqlx::query(&query).fetch_all(&*self.pool).await {
            Ok(rows) => {
                let result: Vec<_> = rows
                    .iter()
                    .map(|row| {
                        let mut obj = serde_json::Map::new();
                        for (i, column) in row.columns().iter().enumerate() {
                            let value: serde_json::Value = match column.type_info().name() {
                                "TEXT" => row
                                    .get::<Option<String>, _>(i)
                                    .map_or(serde_json::Value::Null, serde_json::Value::String),
                                "INTEGER" | "NULL" => row
                                    .get::<Option<i64>, _>(i)
                                    .map_or(serde_json::Value::Null, |n| {
                                        serde_json::Value::Number(n.into())
                                    }),
                                "REAL" => row.get::<Option<f64>, _>(i).map_or(
                                    serde_json::Value::Null,
                                    |f| {
                                        serde_json::Number::from_f64(f).map_or(
                                            serde_json::Value::Null,
                                            serde_json::Value::Number,
                                        )
                                    },
                                ),
                                "BLOB" => row
                                    .get::<Option<Vec<u8>>, _>(i)
                                    .map_or(serde_json::Value::Null, |bytes| {
                                        serde_json::Value::String(STANDARD.encode(bytes))
                                    }),
                                _ => row
                                    .get::<Option<String>, _>(i)
                                    .map_or(serde_json::Value::Null, serde_json::Value::String),
                            };
                            obj.insert(column.name().to_string(), value);
                        }
                        serde_json::Value::Object(obj)
                    })
                    .collect();

                let json = serde_json::to_string(&result).unwrap();

                Response::builder()
                    .status(StatusCode::OK)
                    .header(CONTENT_TYPE, "application/json")
                    .body(Body::from(json))
                    .unwrap()
            }
            Err(e) => Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from(format!("Query error: {:?}", e)))
                .unwrap(),
        }
    }

    fn extract_query(&self, req: Request<Body>) -> Result<String, Response<Body>> {
        match *req.method() {
            Method::GET => {
                let params = req.uri().query().unwrap_or_default();
                Ok(form_urlencoded::parse(params.as_bytes())
                    .find(|(key, _)| key == "q")
                    .map(|(_, value)| value.to_string())
                    .unwrap_or_default())
            }
            Method::POST => {
                // Note: This would need to be adjusted to handle the async body reading
                Ok(String::new()) // Placeholder
            }
            _ => Err(Response::builder()
                .status(StatusCode::METHOD_NOT_ALLOWED)
                .body(Body::from("Only GET and POST methods are allowed"))
                .unwrap()),
        }
    }
}

#[async_trait::async_trait]
impl Handler for SqlHandler {
    fn can_handle(&self, req: &Request<Body>) -> bool {
        req.uri().path().starts_with("/sql")
    }

    async fn handle(&self, req: Request<Body>) -> Response<Body> {
        match self.extract_query(req) {
            Ok(query) => self.execute_query(query).await,
            Err(response) => response,
        }
    }
}

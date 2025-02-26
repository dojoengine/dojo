// crates/katana/cli/src/explorer.rs
use anyhow::{Result, anyhow};
use tiny_http::{Server, Response};
use std::thread;
use tracing::{info, warn};
use katana_explorer::{ExplorerAssets, inject_rpc_url, get_content_type};

pub struct ExplorerServer {
    port: u16,
    rpc_url: String,
}

impl ExplorerServer {
    pub fn new(port: u16, rpc_url: String) -> Result<Self> {
        // Validate that the embedded assets are available
        if ExplorerAssets::get("index.html").is_none() {
            warn!(
                target: "katana",
                "Embedded explorer assets not found. The explorer may not work correctly. \
                Make sure the explorer is built and the dist directory is available."
            );
        }

        Ok(Self {
            port,
            rpc_url,
        })
    }

    pub fn start(&self) -> Result<()> {
        let addr = format!("127.0.0.1:{}", self.port);
        let server = Server::http(&addr)
            .map_err(|e| anyhow!("Failed to start explorer server: {}", e))?;

        info!(
            target: "katana",
            "Starting explorer at addr=http://{}", 
            addr
        );

        let rpc_url = self.rpc_url.clone();

        let _handle = thread::spawn(move || {
            for request in server.incoming_requests() {
                // Special handling for OPTIONS requests (CORS preflight)
                if request.method() == &tiny_http::Method::Options {
                    let response = Response::empty(204)
                        .with_header(tiny_http::Header {
                            field: "Access-Control-Allow-Origin".parse().unwrap(),
                            value: "*".parse().unwrap(),
                        })
                        .with_header(tiny_http::Header {
                            field: "Access-Control-Allow-Methods".parse().unwrap(),
                            value: "GET, POST, OPTIONS".parse().unwrap(),
                        })
                        .with_header(tiny_http::Header {
                            field: "Access-Control-Allow-Headers".parse().unwrap(),
                            value: "Content-Type, Authorization".parse().unwrap(),
                        })
                        .with_header(tiny_http::Header {
                            field: "Access-Control-Max-Age".parse().unwrap(),
                            value: "86400".parse().unwrap(),
                        });
                    let _ = request.respond(response);
                    continue;
                }

                // Decode URL and sanitize path to prevent directory traversal
                let path = {
                    let url_path = request.url().to_string();
                    let decoded_path = urlencoding::decode(&url_path)
                        .map(|s| s.into_owned())
                        .unwrap_or_else(|_| url_path);
                    
                    let p = decoded_path.trim_start_matches('/');
                    if p.is_empty() || p.contains("..") || p.starts_with('/') {
                        "/index.html".to_string()
                    } else {
                        format!("/{}", p)
                    }
                };
                
                // Try to serve from embedded assets
                let content = if let Some(asset) = ExplorerAssets::get(&path[1..]) {
                    let content_type = get_content_type(&path);
                    let content = asset.data;
                    
                    // If it's HTML, inject the RPC URL
                    if content_type == "text/html" {
                        let html = String::from_utf8_lossy(&content).to_string();
                        let html = inject_rpc_url(&html, &rpc_url);
                        Response::from_string(html)
                            .with_header(tiny_http::Header {
                                field: "Content-Type".parse().unwrap(),
                                value: content_type.parse().unwrap(),
                            })
                    } else {
                        Response::from_data(content.to_vec())
                            .with_header(tiny_http::Header {
                                field: "Content-Type".parse().unwrap(),
                                value: content_type.parse().unwrap(),
                            })
                    }
                } else {
                    // Not found
                    Response::from_string("Not found")
                        .with_status_code(404)
                };

                // Add CORS headers
                let response = content
                    .with_header(tiny_http::Header {
                        field: "Access-Control-Allow-Origin".parse().unwrap(),
                        value: "*".parse().unwrap(),
                    })
                    .with_header(tiny_http::Header {
                        field: "Access-Control-Allow-Methods".parse().unwrap(),
                        value: "GET, POST, OPTIONS".parse().unwrap(),
                    })
                    .with_header(tiny_http::Header {
                        field: "Access-Control-Allow-Headers".parse().unwrap(),
                        value: "Content-Type".parse().unwrap(),
                    });
                
                if let Err(e) = request.respond(response) {
                    warn!(target: "katana", "Error sending response: {}", e);
                }
            }
        });
        Ok(())
    }
}
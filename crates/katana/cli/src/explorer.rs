// crates/katana/cli/src/explorer.rs
use std::path::PathBuf;
use anyhow::{Result};
use tiny_http::{Server, Response};
use std::thread;
use tracing::info;

pub struct ExplorerServer {
    port: u16,
    build_dir: PathBuf,
}

impl ExplorerServer {
    pub fn new(port: u16, build_dir: PathBuf) -> Result<Self> {
        Ok(Self {
            port,
            build_dir,
        })
    }

    pub fn start(&self) -> Result<()> {
        // Create the server
        let addr = format!("127.0.0.1:{}", self.port);
        let server = Server::http(&addr)
            .map_err(|e| anyhow::anyhow!("Failed to start explorer server: {}", e))?;

        // Handle requests in a separate thread
        let build_dir = self.build_dir.clone();
        thread::spawn(move || {
            info!(
                target: "katana",
                "Explorer server started. addr=http://{}", 
                addr,
            );

            for request in server.incoming_requests() {
                let path = request.url().to_string();
                info!(
                    target: "katana::explorer",
                    "Received request for: {}", 
                    path
                );

                let file_path = if path == "/" {
                    build_dir.join("index.html")
                } else {
                    build_dir.join(&path[1..])
                };

                if let Ok(content) = std::fs::read(&file_path) {
                    let content_type = match file_path.extension().and_then(|s| s.to_str()) {
                        Some("html") => "text/html",
                        Some("js") => "application/javascript",
                        Some("css") => "text/css",
                        Some("png") => "image/png",
                        Some("svg") => "image/svg+xml",
                        Some("json") => "application/json",
                        _ => "application/octet-stream",
                    };

                    let response = Response::from_data(content)
                        .with_header(tiny_http::Header {
                            field: "Content-Type".parse().unwrap(),
                            value: content_type.parse().unwrap(),
                        })
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
                    
                    let _ = request.respond(response);
                } else {
                    // If file not found, serve index.html for SPA routing
                    if let Ok(content) = std::fs::read(build_dir.join("index.html")) {
                        let response = Response::from_data(content)
                            .with_header(tiny_http::Header {
                                field: "Content-Type".parse().unwrap(),
                                value: "text/html".parse().unwrap(),
                            });
                        let _ = request.respond(response);
                    }
                }
            }
        });

        Ok(())
    }
}
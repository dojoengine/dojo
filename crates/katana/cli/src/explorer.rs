// crates/katana/cli/src/explorer.rs
use std::path::PathBuf;
use anyhow::{Result};
use tiny_http::{Server, Response};
use std::thread;
use tracing::info;

pub struct ExplorerServer {
    port: u16,
    build_dir: PathBuf,
    rpc_url: String,
}

impl ExplorerServer {
    pub fn new(port: u16, build_dir: PathBuf, rpc_url: String) -> Result<Self> {
        Ok(Self {
            port,
            build_dir,
            rpc_url,
        })
    }

    pub fn start(&self) -> Result<()> {
        let addr = format!("127.0.0.1:{}", self.port);
        let server = Server::http(&addr)
            .map_err(|e| anyhow::anyhow!("Failed to start explorer server: {}", e))?;

        info!(
            target: "katana",
            "Starting explorer server. addr=http://{}", 
            addr
        );

        let build_dir = self.build_dir.clone();
        let rpc_url = self.rpc_url.clone();

        thread::spawn(move || {
            for request in server.incoming_requests() {
                let path = request.url().to_string();
                
                let file_path = if path == "/" {
                    build_dir.join("index.html")
                } else {
                    build_dir.join(&path[1..])
                };

                if let Ok(mut content) = std::fs::read_to_string(&file_path) {
                    let content_type = match file_path.extension().and_then(|s| s.to_str()) {
                        Some("html") => {
                            content = inject_rpc_url(&content, &rpc_url);
                            "text/html"
                        },
                        Some("js") => "application/javascript",
                        Some("css") => "text/css",
                        Some("png") => "image/png",
                        Some("svg") => "image/svg+xml",
                        Some("json") => "application/json",
                        _ => "application/octet-stream",
                    };

                    let response = Response::from_string(content)
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
                } else if path == "/" || path.starts_with("/app") {
                    if let Ok(mut content) = std::fs::read_to_string(build_dir.join("index.html")) {
                        content = inject_rpc_url(&content, &rpc_url);
                        let response = Response::from_string(content)
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

fn inject_rpc_url(html: &str, rpc_url: &str) -> String {
    let script = format!(
        r#"<script>
            window.VITE_RPC_URL = "{}";
        </script>"#,
        rpc_url
    );

    if let Some(head_pos) = html.find("<head>") {
        let (start, end) = html.split_at(head_pos + 6);
        format!("{}{}{}", start, script, end)
    } else {
        format!("{}\n{}", script, html)
    }
}
use std::net::SocketAddr;
use std::thread;

use anyhow::{anyhow, Result};
use rust_embed::RustEmbed;
use tiny_http::{Response, Server};
use tracing::{info, warn};
use url::Url;

#[derive(Debug)]
pub struct Explorer {
    /// The JSON-RPC url of the chain that the explorer will connect to.
    rpc_url: Url,
}

impl Explorer {
    pub fn new(rpc_url: Url) -> Result<Self> {
        // Validate that the embedded assets are available
        if ExplorerAssets::get("index.html").is_none() {
            return Err(anyhow!(
                "Explorer assets not found. Make sure the explorer UI is built in CI and the ui/dist directory is available."
            ));
        }

        Ok(Self { rpc_url })
    }

    /// Start the explorer server at the given address.
    pub fn start(&self, addr: SocketAddr) -> Result<()> {
        let server =
            Server::http(addr).map_err(|e| anyhow!("Failed to start explorer server: {}", e))?;

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
                    let components: Vec<&str> = p.split('/').filter(|s| {
                        !s.is_empty() && *s != "." && *s != ".." && !s.contains('\\')
                    }).collect();

                    if components.is_empty() {
                        "/index.html".to_string()
                    } else {
                        format!("/{}", components.join("/"))
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
                        Response::from_string(html).with_header(tiny_http::Header {
                            field: "Content-Type".parse().unwrap(),
                            value: content_type.parse().unwrap(),
                        })
                    } else {
                        Response::from_data(content.to_vec()).with_header(tiny_http::Header {
                            field: "Content-Type".parse().unwrap(),
                            value: content_type.parse().unwrap(),
                        })
                    }
                } else {
                    // Not found
                    Response::from_string("Not found").with_status_code(404)
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

        info!(target: "katana", %addr, "Explorer started.");

        Ok(())
    }
}

/// Embedded explorer UI files.
#[derive(RustEmbed)]
#[folder = "ui/dist"]
struct ExplorerAssets;

/// This function adds a script tag to the HTML that sets the RPC URL
/// for the explorer to use.
fn inject_rpc_url(html: &str, rpc_url: &Url) -> String {
    // Escape special characters to prevent XSS
    let rpc_url = rpc_url.to_string();
    let escaped_url = rpc_url.replace("\"", "\\\"").replace("<", "&lt;").replace(">", "&gt;");

    let script = format!(
        r#"<script>
            window.RPC_URL = "{}";
        </script>"#,
        escaped_url
    );

    if let Some(head_pos) = html.find("<head>") {
        let (start, end) = html.split_at(head_pos + 6);
        format!("{}{}{}", start, script, end)
    } else {
        format!("{}\n{}", script, html)
    }
}

/// Gets the content type for a file based on its extension.
fn get_content_type(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html",
        Some("js") => "application/javascript",
        Some("css") => "text/css",
        Some("png") => "image/png",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("eot") => "application/vnd.ms-fontobject",
        _ => "application/octet-stream",
    }
}

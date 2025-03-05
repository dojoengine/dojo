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
    /// The chain ID of the network
    chain_id: String,
}

impl Explorer {
    pub fn new(rpc_url: Url, chain_id: String) -> Result<Self> {
        // Validate that the embedded assets are available
        if ExplorerAssets::get("index.html").is_none() {
            return Err(anyhow!(
                "Explorer assets not found. Make sure the explorer UI is built in CI and the \
                 ui/dist directory is available."
            ));
        }

        Ok(Self { rpc_url, chain_id })
    }

    // Start the explorer server at the given address.
    pub fn start(&self, addr: SocketAddr) -> Result<ExplorerHandle> {
        let server =
            Server::http(addr).map_err(|e| anyhow!("Failed to start explorer server: {}", e))?;

        let addr = server.server_addr().to_ip().expect("must be ip");
        let rpc_url = self.rpc_url.clone();
        let chain_id = self.chain_id.clone();

        // TODO: handle cancellation
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
                    let components: Vec<&str> = p
                        .split('/')
                        .filter(|s| !s.is_empty() && *s != "." && *s != ".." && !s.contains('\\'))
                        .collect();

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

                    // If it's HTML, inject the RPC URL and chain ID
                    if content_type == "text/html" {
                        let html = String::from_utf8_lossy(&content).to_string();
                        let html = setup_env(&html, &rpc_url, &chain_id);
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

        Ok(ExplorerHandle { addr })
    }
}

/// Handle to the explorer server.
#[derive(Debug)]
pub struct ExplorerHandle {
    addr: SocketAddr,
}

impl ExplorerHandle {
    /// Returns the socket address of the explorer.
    pub fn addr(&self) -> &SocketAddr {
        &self.addr
    }
}

/// Embedded explorer UI files.
#[derive(RustEmbed)]
#[folder = "ui/dist"]
struct ExplorerAssets;

/// This function adds a script tag to the HTML that sets up environment variables
/// for the explorer to use.
fn setup_env(html: &str, rpc_url: &Url, chain_id: &str) -> String {
    // Escape special characters to prevent XSS
    let rpc_url = rpc_url.to_string();
    let escaped_url = rpc_url.replace("\"", "\\\"").replace("<", "&lt;").replace(">", "&gt;");
    let escaped_chain_id = chain_id.replace("\"", "\\\"").replace("<", "&lt;").replace(">", "&gt;");

    // We inject the RPC URL and chain ID into the HTML for the controller to use.
    // The chain rpc and chain id are required params to initialize the controller <https://github.com/cartridge-gg/controller/blob/main/packages/controller/src/controller.ts#L32>.
    // The parameters are consumed by the explorer here <https://github.com/cartridge-gg/explorer/blob/68ac4ea9500a90abc0d7c558440a99587cb77585/src/constants/rpc.ts#L14-L15>. 

    // NOTE: ENABLE_CONTROLLER feature flag is a temporary solution to handle the controller.
    // The controller expects to have a `defaultChainId` but we don't have a way
    // to set it in the explorer yet in development mode (locally running katana instance).
    // The temporary solution is to disable the controller by setting the ENABLE_CONTROLLER flag to
    // false for these explorers. Once we have an updated controller JS SDK which can handle the
    // chain ID of local katana instances then we can remove this flag value. (ref - https://github.com/cartridge-gg/controller/blob/main/packages/controller/src/controller.ts#L57)
    // TODO: remove the ENABLE_CONTROLLER flag once we have a proper way to handle the chain ID for
    // local katana instances.
    let script = format!(
        r#"<script>
            window.RPC_URL = "{}";
            window.CHAIN_ID = "{}";
            window.ENABLE_CONTROLLER = false;
        </script>"#,
        escaped_url, escaped_chain_id
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

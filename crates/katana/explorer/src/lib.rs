//! Embedded explorer UI for Katana.
use rust_embed::RustEmbed;

/// Embedded explorer UI files.
#[derive(RustEmbed)]
#[folder = "dist"]
pub struct ExplorerAssets;

/// This function adds a script tag to the HTML that sets the RPC URL
/// for the explorer to use.
pub fn inject_rpc_url(html: &str, rpc_url: &str) -> String {
    // Escape special characters to prevent XSS
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
pub fn get_content_type(path: &str) -> &'static str {
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
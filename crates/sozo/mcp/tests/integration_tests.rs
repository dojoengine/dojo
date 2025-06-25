use anyhow::Result;
use serde_json::json;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::timeout;

const SPAWN_AND_MOVE_MANIFEST_PATH: &str = "./examples/spawn-and-move/Scarb.toml";

/// Helper struct to manage MCP server process.
struct McpServerProcess {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: std::process::ChildStdout,
}

impl McpServerProcess {
    /// Spawns the MCP server process with the given manifest path.
    fn new(manifest_path: &str) -> Result<Self> {
        let mut cmd = Command::new("cargo");
        cmd.args(["run", "--bin", "sozo", "mcp", "--manifest-path", manifest_path])
            .current_dir("../../../")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        Ok(Self { child, stdin, stdout })
    }

    /// Sends a request to the MCP server and returns the response.
    async fn send_request(&mut self, request: serde_json::Value) -> Result<serde_json::Value> {
        let request_str = serde_json::to_string(&request)?;
        let request_with_newline = format!("{}\n", request_str);
        
        use std::io::Write;
        self.stdin.write_all(request_with_newline.as_bytes())?;
        self.stdin.flush()?;

        let mut reader = std::io::BufReader::new(&mut self.stdout);
        use std::io::BufRead;
        
        let mut line = String::new();
        reader.read_line(&mut line)?;

        let response: serde_json::Value = serde_json::from_str(line.trim())?;
        Ok(response)
    }

    /// Initializes the MCP server with the first request.
    async fn initialize(&mut self) -> Result<()> {
        let init_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {
                    "tools": {},
                    "resources": {}
                },
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            }
        });

        let response = timeout(Duration::from_secs(5), self.send_request(init_request)).await??;
        
        assert!(response.get("result").is_some());
        let result = response["result"].as_object().unwrap();
        assert_eq!(result["protocolVersion"], "2025-03-26");

        // Send initialized notification
        let init_notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let notification_str = serde_json::to_string(&init_notification)?;
        let notification_with_newline = format!("{}\n", notification_str);
        
        use std::io::Write;
        self.stdin.write_all(notification_with_newline.as_bytes())?;
        self.stdin.flush()?;

        Ok(())
    }

    fn cleanup(&mut self) -> Result<()> {
        self.child.kill()?;
        self.child.wait()?;
        Ok(())
    }
}

/// Tests server initialization via STDIO.
/// Should return the server info.
#[tokio::test]
async fn test_server_initialization_stdio() -> Result<()> {
    let mut server = McpServerProcess::new(SPAWN_AND_MOVE_MANIFEST_PATH)?;

    // Initialize the connection
    server.initialize().await?;
    
    server.cleanup()?;
    Ok(())
}

/// Tests list resources via STDIO.
/// Should return the list of resources.
#[tokio::test]
async fn test_list_resources_stdio() -> Result<()> {
    let mut server = McpServerProcess::new(SPAWN_AND_MOVE_MANIFEST_PATH)?;

    server.initialize().await?;

    let list_request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "resources/list",
        "params": {}
    });

    let response = timeout(Duration::from_secs(5), server.send_request(list_request)).await??;
    
    assert!(response.get("result").is_some());
    let result = response["result"].as_object().unwrap();
    let resources = result["resources"].as_array().unwrap();
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0]["uri"], "dojo://scarb/manifest");
    
    server.cleanup()?;
    Ok(())
}

/// Tests list resource templates via STDIO.
/// Should return the list of resource templates.
#[tokio::test]
async fn test_list_resource_templates_stdio() -> Result<()> {
    let mut server = McpServerProcess::new(SPAWN_AND_MOVE_MANIFEST_PATH)?;

    server.initialize().await?;

    let list_request = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "resources/templates/list",
        "params": {}
    });

    let response = timeout(Duration::from_secs(5), server.send_request(list_request)).await??;
    
    assert!(response.get("result").is_some());
    let result = response["result"].as_object().unwrap();
    let templates = result["resourceTemplates"].as_array().unwrap();
    assert_eq!(templates.len(), 3);
    
    let template_uris: Vec<&str> = templates
        .iter()
        .map(|t| t["uriTemplate"].as_str().unwrap())
        .collect();
    
    assert!(template_uris.contains(&"dojo://contract/{profile}/{name}/abi"));
    assert!(template_uris.contains(&"dojo://model/{profile}/{name}/abi"));
    assert!(template_uris.contains(&"dojo://event/{profile}/{name}/abi"));
    
    server.cleanup()?;
    Ok(())
}

/// Tests list tools via STDIO.
/// Should return the list of tools.
#[tokio::test]
async fn test_list_tools_stdio() -> Result<()> {
    let mut server = McpServerProcess::new(SPAWN_AND_MOVE_MANIFEST_PATH)?;

    server.initialize().await?;

    let list_request = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/list",
        "params": {}
    });

    let response = timeout(Duration::from_secs(5), server.send_request(list_request)).await??;
    
    assert!(response.get("result").is_some());
    let result = response["result"].as_object().unwrap();
    let tools = result["tools"].as_array().unwrap();
    
    let tool_names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    
    assert!(tool_names.contains(&"build"));
    assert!(tool_names.contains(&"test"));
    assert!(tool_names.contains(&"inspect"));
    assert!(tool_names.contains(&"migrate"));
    assert!(tool_names.contains(&"execute"));
    
    server.cleanup()?;
    Ok(())
}

/// Tests read manifest resource via STDIO.
/// Should return the TOML manifest as JSON.
#[tokio::test]
async fn test_read_manifest_resource_stdio() -> Result<()> {
    let mut server = McpServerProcess::new(SPAWN_AND_MOVE_MANIFEST_PATH)?;

    server.initialize().await?;

    let read_request = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "resources/read",
        "params": {
            "uri": "dojo://scarb/manifest"
        }
    });

    let response = timeout(Duration::from_secs(5), server.send_request(read_request)).await??;

    assert!(response.get("result").is_some());
    let result = response.get("result").unwrap();
    
    let text_field = result["contents"][0]["text"].as_str().unwrap();
    println!("Manifest JSON content: {}", text_field);
    
    let manifest_json: serde_json::Value = serde_json::from_str(text_field)?;
    
    assert!(manifest_json["package"].is_object());
    assert_eq!(manifest_json["package"]["name"], "dojo_examples");
    assert_eq!(manifest_json["package"]["version"], "1.5.1");
    assert!(manifest_json["dependencies"].is_object());
    assert!(manifest_json["dependencies"]["dojo"].is_object());
    
    server.cleanup()?;
    Ok(())
}

/// Tests read manifest resource with valid manifest path via STDIO.
#[tokio::test]
async fn test_read_manifest_resource_with_path_stdio() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let manifest_path = temp_dir.path().join("Scarb.toml");
    
    let manifest_content = r#"
[package]
name = "test_project"
version = "0.1.0"

[dependencies]
dojo = { git = "https://github.com/dojoengine/dojo" }
"#;
    
    std::fs::write(&manifest_path, manifest_content)?;
    
    let mut server = McpServerProcess::new(manifest_path.to_str().unwrap())?;

    server.initialize().await?;

    let read_request = json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "resources/read",
        "params": {
            "uri": "dojo://scarb/manifest"
        }
    });

    let response = timeout(Duration::from_secs(5), server.send_request(read_request)).await??;
    
    assert!(response.get("result").is_some());
    let result = response.get("result").unwrap();
    
    let text_field = result["contents"][0]["text"].as_str().unwrap();
    println!("Temporary manifest JSON content: {}", text_field);
    
    let manifest_json: serde_json::Value = serde_json::from_str(text_field)?;
    
    assert!(manifest_json["package"].is_object());
    assert_eq!(manifest_json["package"]["name"], "test_project");
    assert_eq!(manifest_json["package"]["version"], "0.1.0");
    assert!(manifest_json["dependencies"].is_object());
    assert!(manifest_json["dependencies"]["dojo"]["git"].is_string());
    
    server.cleanup()?;
    Ok(())
}

/// Test call tool via STDIO
#[tokio::test]
async fn test_call_tool_stdio() -> Result<()> {
    let mut server = McpServerProcess::new(SPAWN_AND_MOVE_MANIFEST_PATH)?;

    server.initialize().await?;

    let call_request = json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "build",
            "arguments": {
                "profile": "dev"
            }
        }
    });

    let response = timeout(Duration::from_secs(10), server.send_request(call_request)).await??;

    assert!(response.get("result").is_some());
    
    server.cleanup()?;
    Ok(())
}

/// Tests read contract ABI resource via STDIO.
#[tokio::test]
async fn test_read_contract_abi_stdio() -> Result<()> {
    let mut server = McpServerProcess::new(SPAWN_AND_MOVE_MANIFEST_PATH)?;

    server.initialize().await?;

    let read_request = json!({
        "jsonrpc": "2.0",
        "id": 8,
        "method": "resources/read",
        "params": {
            "uri": "dojo://contract/dev/actions/abi"
        }
    });

    let response = timeout(Duration::from_secs(5), server.send_request(read_request)).await??;

    dbg!(&response);
    
    // This should either succeed with ABI content or fail gracefully
    if response.get("result").is_some() {
        let result = response.get("result").unwrap();
        let text_field = result["contents"][0]["text"].as_str().unwrap();
        println!("Contract ABI content: {}", text_field);
        
        // Parse the JSON content to verify it's valid ABI
        let abi_json: serde_json::Value = serde_json::from_str(text_field)?;
        assert!(abi_json.is_array() || abi_json.is_object());
    } else {
        // If it fails, it should be a proper error response
        assert!(response.get("error").is_some());
        println!("Contract ABI not available: {:?}", response.get("error"));
    }
    
    server.cleanup()?;
    Ok(())
}

/// Test multiple requests in sequence
#[tokio::test]
async fn test_multiple_requests_stdio() -> Result<()> {
    let mut server = McpServerProcess::new(SPAWN_AND_MOVE_MANIFEST_PATH)?;

    // Initialize the connection first
    server.initialize().await?;

    // List resources
    let list_request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "resources/list",
        "params": {}
    });

    let list_response = timeout(Duration::from_secs(5), server.send_request(list_request)).await??;
    assert!(list_response.get("result").is_some());

    // List tools
    let tools_request = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/list",
        "params": {}
    });

    let tools_response = timeout(Duration::from_secs(5), server.send_request(tools_request)).await??;
    assert!(tools_response.get("result").is_some());
    
    server.cleanup()?;
    Ok(())
}

/// Test call build tool via STDIO
#[tokio::test]
async fn test_call_build_tool_stdio() -> Result<()> {
    let mut server = McpServerProcess::new(SPAWN_AND_MOVE_MANIFEST_PATH)?;

    // Initialize the connection first
    server.initialize().await?;

    let call_request = json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "tools/call",
        "params": {
            "name": "build",
            "arguments": {
                "profile": "dev"
            }
        }
    });

    let response = timeout(Duration::from_secs(30), server.send_request(call_request)).await??;
    
    // The build tool should be called successfully
    assert!(response.get("result").is_some());
    let result = response.get("result").unwrap();
    
    // Verify the response structure
    assert!(result["content"].is_array());
    let content = result["content"].as_array().unwrap();
    assert!(!content.is_empty());
    
    // The first content item should have text
    let first_content = &content[0];
    assert!(first_content["text"].is_string());
    
    println!("Build tool output: {}", first_content["text"]);
    
    server.cleanup()?;
    Ok(())
}

/// Test call inspect tool via STDIO
#[tokio::test]
async fn test_call_inspect_tool_stdio() -> Result<()> {
    let mut server = McpServerProcess::new(SPAWN_AND_MOVE_MANIFEST_PATH)?;

    // Initialize the connection first
    server.initialize().await?;

    let call_request = json!({
        "jsonrpc": "2.0",
        "id": 10,
        "method": "tools/call",
        "params": {
            "name": "inspect",
            "arguments": {
                "profile": "dev"
            }
        }
    });

    let response = timeout(Duration::from_secs(60), server.send_request(call_request)).await??;
    
    // The inspect tool should be called successfully
    assert!(response.get("result").is_some());
    let result = response.get("result").unwrap();
    
    // Verify the response structure
    assert!(result["content"].is_array());
    let content = result["content"].as_array().unwrap();
    assert!(!content.is_empty());
    
    // The first content item should have text
    let first_content = &content[0];
    assert!(first_content["text"].is_string());
    
    let inspect_output = first_content["text"].as_str().unwrap();
    println!("Inspect tool output: {}", inspect_output);
    
    // Verify it contains expected project information
    assert!(inspect_output.contains("dojo_examples"));
    assert!(inspect_output.contains("World"));
    
    server.cleanup()?;
    Ok(())
} 
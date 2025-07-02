use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::Result;
use dojo_test_utils::setup::TestSetup;
use scarb_interop::Profile;
use serde_json::json;
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

    let template_uris: Vec<&str> =
        templates.iter().map(|t| t["uriTemplate"].as_str().unwrap()).collect();

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

    let tool_names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

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
    assert_eq!(manifest_json["package"]["version"], "1.6.0-alpha.0");
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

    if response.get("result").is_some() {
        let result = response.get("result").unwrap();
        let text_field = result["contents"][0]["text"].as_str().unwrap();
        println!("Contract ABI content: {}", text_field);

        let abi_json: serde_json::Value = serde_json::from_str(text_field)?;
        assert!(abi_json.is_array() || abi_json.is_object());
    } else {
        assert!(response.get("error").is_some());
        println!("Contract ABI not available: {:?}", response.get("error"));
    }

    server.cleanup()?;
    Ok(())
}

/// Tests multiple requests in sequence.
#[tokio::test]
async fn test_multiple_requests_stdio() -> Result<()> {
    let mut server = McpServerProcess::new(SPAWN_AND_MOVE_MANIFEST_PATH)?;

    server.initialize().await?;

    let list_request = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "resources/list",
        "params": {}
    });

    let list_response =
        timeout(Duration::from_secs(5), server.send_request(list_request)).await??;
    assert!(list_response.get("result").is_some());

    let tools_request = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/list",
        "params": {}
    });

    let tools_response =
        timeout(Duration::from_secs(5), server.send_request(tools_request)).await??;
    assert!(tools_response.get("result").is_some());

    server.cleanup()?;
    Ok(())
}

/// Tests call build tool via STDIO.
#[tokio::test]
async fn test_call_build_tool_stdio() -> Result<()> {
    let mut server = McpServerProcess::new(SPAWN_AND_MOVE_MANIFEST_PATH)?;

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

    assert!(response.get("result").is_some());
    let result = response.get("result").unwrap();
    // Verify the response structure
    assert!(result["content"].is_array());
    let content = result["content"].as_array().unwrap();
    assert!(!content.is_empty());

    let first_content = &content[0];
    assert!(first_content["text"].is_string());

    println!("Build tool output: {}", first_content["text"]);

    server.cleanup()?;
    Ok(())
}

/// Tests call inspect tool via STDIO.
/// Important, this test is only valid if the project has been built.
/// In the CI, the project is built before the tests are run, but if run locally,
/// ensures that the project is built before the test is run.
#[tokio::test]
#[ignore = "This test require a Katana to be setup and running on the port that is mentioned by \
            the configuration. Currently only used locally for debugging."]
async fn test_call_inspect_tool_stdio() -> Result<()> {
    let mut server = McpServerProcess::new(SPAWN_AND_MOVE_MANIFEST_PATH)?;

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

    let response = timeout(Duration::from_secs(30), server.send_request(call_request)).await??;

    assert!(response.get("result").is_some());
    let result = response.get("result").unwrap();

    assert!(result["content"].is_array());
    let content = result["content"].as_array().unwrap();
    assert!(!content.is_empty());

    let first_content = &content[0];
    assert!(first_content["text"].is_string());

    let inspect_output = first_content["text"].as_str().unwrap();

    // Parse the output as JSON
    let inspect_json: serde_json::Value = serde_json::from_str(inspect_output)?;

    // Verify the expected top-level keys
    assert!(inspect_json["contracts"].is_array());
    assert!(inspect_json["events"].is_array());
    assert!(inspect_json["external_contracts"].is_array());
    assert!(inspect_json["libraries"].is_array());
    assert!(inspect_json["models"].is_array());
    assert!(inspect_json["namespaces"].is_array());
    assert!(inspect_json["world"].is_object());

    // Verify contracts array has expected structure
    let contracts = inspect_json["contracts"].as_array().unwrap();
    assert!(!contracts.is_empty());

    for contract in contracts {
        assert!(contract["address"].is_string());
        assert!(contract["class_hash"].is_string());
        assert!(contract["is_initialized"].is_boolean());
        assert!(contract["selector"].is_string());
        assert!(contract["status"].is_string());
        assert!(contract["tag"].is_string());
    }

    let events = inspect_json["events"].as_array().unwrap();
    assert!(!events.is_empty());

    for event in events {
        assert!(event["selector"].is_string());
        assert!(event["status"].is_string());
        assert!(event["tag"].is_string());
    }

    let external_contracts = inspect_json["external_contracts"].as_array().unwrap();
    assert!(!external_contracts.is_empty());

    for ext_contract in external_contracts {
        assert!(ext_contract["address"].is_string());
        assert!(ext_contract["class_hash"].is_string());
        assert!(ext_contract["constructor_calldata"].is_array());
        assert!(ext_contract["contract_name"].is_string());
        assert!(ext_contract["instance_name"].is_string());
        assert!(ext_contract["salt"].is_string());
        assert!(ext_contract["status"].is_string());
    }

    let libraries = inspect_json["libraries"].as_array().unwrap();
    assert!(!libraries.is_empty());

    for library in libraries {
        assert!(library["class_hash"].is_string());
        assert!(library["selector"].is_string());
        assert!(library["status"].is_string());
        assert!(library["tag"].is_string());
        assert!(library["version"].is_string());
    }

    let models = inspect_json["models"].as_array().unwrap();
    assert!(!models.is_empty());

    for model in models {
        assert!(model["selector"].is_string());
        assert!(model["status"].is_string());
        assert!(model["tag"].is_string());
    }

    let namespaces = inspect_json["namespaces"].as_array().unwrap();
    assert!(!namespaces.is_empty());

    for namespace in namespaces {
        assert!(namespace["name"].is_string());
        assert!(namespace["selector"].is_string());
        assert!(namespace["status"].is_string());
    }

    let world = &inspect_json["world"];
    assert!(world["address"].is_string());
    assert!(world["class_hash"].is_string());
    assert!(world["status"].is_string());

    server.cleanup()?;
    Ok(())
}

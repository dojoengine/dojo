#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use camino::Utf8PathBuf;
    use rmcp::model::*;
    use serde_json::json;

    /// Test server info
    #[test]
    fn test_server_info() {
        let server = SozoMcpServer::new(None);
        let info = server.get_info();

        assert_eq!(info.protocol_version, ProtocolVersion::V_2025_03_26);
        assert!(info.capabilities.tools);
        assert!(info.capabilities.resources);
        assert!(info.instructions.is_some());
        assert!(info.instructions.unwrap().contains("Sozo MCP Server"));
    }

    /// Test list resources
    #[tokio::test]
    async fn test_list_resources() -> Result<()> {
        let server = SozoMcpServer::new(None);
        let context = RequestContext::new(RoleServer);

        let result = server.list_resources(None, context).await?;
        
        assert_eq!(result.resources.len(), 1);
        assert_eq!(result.resources[0].uri, "dojo://scarb/manifest");
        assert_eq!(result.resources[0].name, "Scarb project manifest");

        Ok(())
    }

    /// Test list resource templates
    #[tokio::test]
    async fn test_list_resource_templates() -> Result<()> {
        let server = SozoMcpServer::new(None);
        let context = RequestContext::new(RoleServer);

        let result = server.list_resource_templates(None, context).await?;
        
        assert_eq!(result.resource_templates.len(), 3);
        
        let contract_template = &result.resource_templates[0];
        assert_eq!(contract_template.uri_template, "dojo://contract/{profile}/{name}/abi");
        assert_eq!(contract_template.name, "Contract ABI");

        let model_template = &result.resource_templates[1];
        assert_eq!(model_template.uri_template, "dojo://model/{profile}/{name}/abi");
        assert_eq!(model_template.name, "Model ABI");

        let event_template = &result.resource_templates[2];
        assert_eq!(event_template.uri_template, "dojo://event/{profile}/{name}/abi");
        assert_eq!(event_template.name, "Event ABI");

        Ok(())
    }

    /// Test read manifest resource
    #[tokio::test]
    async fn test_read_manifest_resource() -> Result<()> {
        let server = SozoMcpServer::new(None);
        let context = RequestContext::new(RoleServer);

        let request = ReadResourceRequestParam {
            uri: "dojo://scarb/manifest".to_string(),
        };

        let result = server.read_resource(request, context).await;
        
        // This should fail without a manifest path, but we can test the error handling
        assert!(result.is_err());

        Ok(())
    }

    /// Test build tool request structure
    #[test]
    fn test_build_request_structure() {
        let request = BuildRequest {
            profile: Some("dev".to_string()),
            manifest_path: None,
        };

        assert_eq!(request.profile, Some("dev".to_string()));
        assert_eq!(request.manifest_path, None);
    }

    /// Test test tool request structure
    #[test]
    fn test_test_request_structure() {
        let request = TestRequest {
            profile: Some("dev".to_string()),
            manifest_path: None,
        };

        assert_eq!(request.profile, Some("dev".to_string()));
        assert_eq!(request.manifest_path, None);
    }

    /// Test inspect tool request structure
    #[test]
    fn test_inspect_request_structure() {
        let request = InspectRequest {
            profile: Some("dev".to_string()),
            manifest_path: None,
        };

        assert_eq!(request.profile, Some("dev".to_string()));
        assert_eq!(request.manifest_path, None);
    }

    /// Test migrate tool request structure
    #[test]
    fn test_migrate_request_structure() {
        let request = MigrateRequest {
            profile: Some("dev".to_string()),
            manifest_path: None,
        };

        assert_eq!(request.profile, Some("dev".to_string()));
        assert_eq!(request.manifest_path, None);
    }

    /// Test execute tool request structure
    #[test]
    fn test_execute_request_structure() {
        let request = ExecuteRequest {
            profile: Some("dev".to_string()),
            contract: "my_namespace-my_contract".to_string(),
            function_name: "spawn".to_string(),
            calldata: vec!["u256:100".to_string(), "str:'player_name'".to_string()],
            manifest_path: None,
        };

        assert_eq!(request.profile, Some("dev".to_string()));
        assert_eq!(request.contract, "my_namespace-my_contract");
        assert_eq!(request.function_name, "spawn");
        assert_eq!(request.calldata.len(), 2);
        assert_eq!(request.manifest_path, None);
    }

    /// Test resource URI parsing
    #[test]
    fn test_resource_uri_parsing() {
        use crate::resources::parser::parse_contract_uri;

        // Test contract URI
        let uri = "dojo://contract/dev/my_contract/abi";
        let parsed = parse_dojo_uri(uri).unwrap();
        assert_eq!(parsed.resource_type, "contract");
        assert_eq!(parsed.profile, "dev");
        assert_eq!(parsed.name, "my_contract");
        assert_eq!(parsed.subpath, "abi");

        // Test model URI
        let uri = "dojo://model/release/my_model/abi";
        let parsed = parse_dojo_uri(uri).unwrap();
        assert_eq!(parsed.resource_type, "model");
        assert_eq!(parsed.profile, "release");
        assert_eq!(parsed.name, "my_model");
        assert_eq!(parsed.subpath, "abi");

        // Test event URI
        let uri = "dojo://event/dev/my_event/abi";
        let parsed = parse_dojo_uri(uri).unwrap();
        assert_eq!(parsed.resource_type, "event");
        assert_eq!(parsed.profile, "dev");
        assert_eq!(parsed.name, "my_event");
        assert_eq!(parsed.subpath, "abi");

        // Test manifest URI
        let uri = "dojo://scarb/manifest";
        let parsed = parse_dojo_uri(uri).unwrap();
        assert_eq!(parsed.resource_type, "scarb");
        assert_eq!(parsed.profile, "");
        assert_eq!(parsed.name, "manifest");
        assert_eq!(parsed.subpath, "");

        // Test invalid URI
        let uri = "invalid://uri";
        assert!(parse_dojo_uri(uri).is_err());
    }

    /// Test TOML to JSON conversion
    #[test]
    fn test_toml_to_json_conversion() {
        use crate::resources::abi::toml_to_json;

        let toml_content = r#"
[package]
name = "test_project"
version = "0.1.0"

[dependencies]
dojo = { git = "https://github.com/dojoengine/dojo" }
"#;

        let json_result = toml_to_json(toml_content).unwrap();
        let json_value: serde_json::Value = serde_json::from_str(&json_result).unwrap();

        assert_eq!(json_value["package"]["name"], "test_project");
        assert_eq!(json_value["package"]["version"], "0.1.0");
        assert!(json_value["dependencies"]["dojo"]["git"].is_string());
    }

    /// Test TOML to JSON conversion with invalid TOML
    #[test]
    fn test_toml_to_json_invalid_toml() {
        use crate::resources::abi::toml_to_json;

        let invalid_toml = "[package\nname = test_project";
        let result = toml_to_json(invalid_toml);
        assert!(result.is_err());
    }

    /// Integration test with actual manifest path
    #[tokio::test]
    async fn test_with_manifest_path() -> Result<()> {
        // Create a temporary manifest for testing
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
        
        let manifest_path_utf8 = Utf8PathBuf::from_path_buf(manifest_path).unwrap();
        let server = SozoMcpServer::new(Some(manifest_path_utf8));
        let context = RequestContext::new(RoleServer);

        let request = ReadResourceRequestParam {
            uri: "dojo://scarb/manifest".to_string(),
        };

        let result = server.read_resource(request, context).await;
        
        // This should succeed with a valid manifest path
        assert!(result.is_ok());

        Ok(())
    }

    /// Test error handling for non-existent manifest
    #[tokio::test]
    async fn test_error_handling_nonexistent_manifest() -> Result<()> {
        let server = SozoMcpServer::new(Some(Utf8PathBuf::from("/nonexistent/path/Scarb.toml")));
        let context = RequestContext::new(RoleServer);

        let request = ReadResourceRequestParam {
            uri: "dojo://scarb/manifest".to_string(),
        };

        let result = server.read_resource(request, context).await;
        
        // This should fail with a non-existent manifest path
        assert!(result.is_err());

        Ok(())
    }

    /// Test tool router functionality
    #[test]
    fn test_tool_router() {
        let server = SozoMcpServer::new(None);
        let router = &server.tool_router;

        // Test that all expected tools are registered
        let tools = router.list_tools();
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        
        assert!(tool_names.contains(&"build"));
        assert!(tool_names.contains(&"test"));
        assert!(tool_names.contains(&"inspect"));
        assert!(tool_names.contains(&"migrate"));
        assert!(tool_names.contains(&"execute"));
    }

    /// Test server capabilities
    #[test]
    fn test_server_capabilities() {
        let server = SozoMcpServer::new(None);
        let info = server.get_info();

        assert!(info.capabilities.tools);
        assert!(info.capabilities.resources);
        assert!(!info.capabilities.prompts);
    }

    /// Test server implementation info
    #[test]
    fn test_server_implementation_info() {
        let server = SozoMcpServer::new(None);
        let info = server.get_info();

        assert_eq!(info.server_info.name, "sozo-mcp");
        assert!(!info.server_info.version.is_empty());
    }

    /// Test server initialization
    #[tokio::test]
    async fn test_server_initialization() -> Result<()> {
        let server = SozoMcpServer::new(None);
        let context = RequestContext::new(RoleServer);

        let request = InitializeRequestParam {
            protocol_version: ProtocolVersion::V_2025_03_26,
            capabilities: ClientCapabilities::builder().build(),
            client_info: Some(ClientInfo {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            }),
        };

        let result = server.initialize(request, context).await?;
        
        assert_eq!(result.protocol_version, ProtocolVersion::V_2025_03_26);
        assert!(result.capabilities.tools);
        assert!(result.capabilities.resources);

        Ok(())
    }

    /// Test server initialization with different protocol version
    #[tokio::test]
    async fn test_server_initialization_different_protocol() -> Result<()> {
        let server = SozoMcpServer::new(None);
        let context = RequestContext::new(RoleServer);

        let request = InitializeRequestParam {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ClientCapabilities::builder().build(),
            client_info: Some(ClientInfo {
                name: "test-client".to_string(),
                version: "1.0.0".to_string(),
            }),
        };

        let result = server.initialize(request, context).await?;
        
        // Should still return the server's protocol version
        assert_eq!(result.protocol_version, ProtocolVersion::V_2025_03_26);

        Ok(())
    }

    /// Test server initialization without client info
    #[tokio::test]
    async fn test_server_initialization_no_client_info() -> Result<()> {
        let server = SozoMcpServer::new(None);
        let context = RequestContext::new(RoleServer);

        let request = InitializeRequestParam {
            protocol_version: ProtocolVersion::V_2025_03_26,
            capabilities: ClientCapabilities::builder().build(),
            client_info: None,
        };

        let result = server.initialize(request, context).await?;
        
        assert_eq!(result.protocol_version, ProtocolVersion::V_2025_03_26);
        assert!(result.capabilities.tools);
        assert!(result.capabilities.resources);

        Ok(())
    }

    /// Test prompts functionality
    #[tokio::test]
    async fn test_prompts() -> Result<()> {
        let server = SozoMcpServer::new(None);
        let context = RequestContext::new(RoleServer);

        // Test list prompts
        let list_result = server.list_prompts(None, context.clone()).await?;
        assert_eq!(list_result.prompts.len(), 0);

        // Test get prompt
        let get_request = GetPromptRequestParam {
            name: "test_prompt".to_string(),
            arguments: None,
        };
        let get_result = server.get_prompt(get_request, context).await?;
        assert!(get_result.messages.is_empty());

        Ok(())
    }
} 
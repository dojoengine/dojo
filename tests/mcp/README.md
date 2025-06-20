# MCP Tests

This directory contains test files for testing the Model Context Protocol (MCP) server functionality.

## Test Files

### `test_resources_list.json`
Tests the basic resource listing functionality:
- Initializes the MCP connection
- Lists available resources

### `test_inspect_tool.json`
Tests the inspect tool with profile arguments:
- Initializes the MCP connection
- Lists available tools
- Calls the inspect tool with `profile: "dev"` and `json: true`

## Usage

To run these tests against the Sozo MCP server:

```bash
# Test resources list
cargo run --bin sozo -r -- mcp --manifest-path examples/spawn-and-move/Scarb.toml < tests/mcp/test_resources_list.json

# Test inspect tool
cargo run --bin sozo -r -- mcp --manifest-path examples/spawn-and-move/Scarb.toml < tests/mcp/test_inspect_tool.json
```

## Expected Behavior

### Resources List Test
- Should return a successful initialization response
- Should return a list of available resources (likely empty for basic setup)

### Inspect Tool Test
- Should return a successful initialization response
- Should return a list of available tools including "inspect"
- Should return JSON output from the inspect command with world information

## Adding New Tests

To add new tests:
1. Create a new `.json` file in this directory
2. Follow the JSON-RPC format with proper initialization sequence
3. Include the `notifications/initialized` message after initialization
4. Add your test requests with unique IDs
5. Document the expected behavior in this README

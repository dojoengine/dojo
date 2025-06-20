#!/bin/bash

# Start the MCP server as a coprocess
coproc MCP {
  cargo run --bin sozo -r -- mcp --manifest-path examples/spawn-and-move/Scarb.toml
}

# Give the server a moment to start
sleep 2

# Send requests
echo '{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test-client", "version": "1.0.0"}}}' >&"${MCP[1]}"
echo '{"jsonrpc": "2.0", "method": "notifications/initialized"}' >&"${MCP[1]}"
echo '{"jsonrpc": "2.0", "id": 2, "method": "tools/list"}' >&"${MCP[1]}"
echo '{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "inspect", "arguments": {"profile": "dev"}}}' >&"${MCP[1]}"

# Read responses (for demonstration, read 4 lines)
for i in {1..4}; do
  read -r line <&"${MCP[0]}"
  echo "Response: $line"
done

# Clean up
kill $COPROC_PID 
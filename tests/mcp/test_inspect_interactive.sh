#!/bin/bash

# Test script for MCP inspect tool
# This script sends requests one by one and waits for responses

echo "Testing MCP inspect tool..."

# Start the MCP server in the background
cargo run --bin sozo -r -- mcp --manifest-path examples/spawn-and-move/Scarb.toml &
MCP_PID=$!

# Wait a moment for the server to start
sleep 2

# Function to send a request and wait for response
send_request() {
    local request="$1"
    echo "$request" >&3
    # Read response (you might need to adjust this based on the actual response format)
    read -r response <&4
    echo "Response: $response"
}

# Open file descriptors for communication
exec 3>&1 4<&0

# Send initialization
echo '{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test-client", "version": "1.0.0"}}}'

# Wait for response
sleep 1

# Send initialized notification
echo '{"jsonrpc": "2.0", "method": "notifications/initialized"}'

# Wait for response
sleep 1

# Send tools/list request
echo '{"jsonrpc": "2.0", "id": 2, "method": "tools/list"}'

# Wait for response
sleep 1

# Send inspect tool call
echo '{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "inspect", "arguments": {"profile": "dev"}}}'

# Wait for the inspect command to complete
sleep 10

# Clean up
kill $MCP_PID 2>/dev/null
wait $MCP_PID 2>/dev/null

echo "Test completed." 
#!/usr/bin/env python3
"""
MCP Client Test Script
Tests the MCP server by sending requests and waiting for responses.
"""

import json
import subprocess
import time
import sys
import select

def send_request(process, request):
    """Send a JSON-RPC request to the MCP server."""
    request_str = json.dumps(request) + "\n"
    process.stdin.write(request_str)
    process.stdin.flush()
    print(f"Sent: {request_str.strip()}")

def read_response(process, timeout=30):
    """Read a response from the MCP server, with a timeout."""
    start_time = time.time()
    while True:
        if process.stdout in select.select([process.stdout], [], [], 1)[0]:
            line = process.stdout.readline().strip()
            if line:
                print(f"Received: {line}")
                try:
                    return json.loads(line)
                except json.JSONDecodeError:
                    print(f"Failed to parse response: {line}")
                    continue
        if time.time() - start_time > timeout:
            print("Timeout waiting for response.")
            return None

def test_mcp_server():
    """Test the MCP server with inspect tool."""
    
    # Start the MCP server
    cmd = [
        "cargo", "run", "--bin", "sozo", "-r", "--", 
        "mcp", "--manifest-path", "examples/spawn-and-move/Scarb.toml"
    ]
    
    print("Starting MCP server...")
    process = subprocess.Popen(
        cmd,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1
    )
    
    try:
        # Wait a moment for server to start
        time.sleep(2)
        
        # Send initialization request
        init_request = {
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "test-client", "version": "1.0.0"}
            }
        }
        send_request(process, init_request)
        
        # Wait for initialization response
        response = read_response(process)
        if not response:
            print("No initialization response received")
            return
        
        # Send initialized notification
        init_notification = {
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }
        send_request(process, init_notification)
        
        # Wait a moment
        time.sleep(1)
        
        # Send tools/list request
        tools_list_request = {
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list"
        }
        send_request(process, tools_list_request)
        
        # Wait for tools list response
        response = read_response(process)
        if not response:
            print("No tools list response received")
            return
        
        # Wait a moment
        time.sleep(1)
        
        # Send inspect tool call
        inspect_request = {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "inspect",
                "arguments": {"profile": "dev"}
            }
        }
        send_request(process, inspect_request)
        
        # Wait for inspect response (this might take a while)
        print("Waiting for inspect response...")
        response = read_response(process, timeout=60)
        if response:
            print("Inspect response received successfully!")
        else:
            print("No inspect response received")
        
    except Exception as e:
        print(f"Error during test: {e}")
    finally:
        # Clean up
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        
        # Print any stderr output
        stderr_output = process.stderr.read()
        if stderr_output:
            print(f"Stderr: {stderr_output}")

if __name__ == "__main__":
    test_mcp_server() 
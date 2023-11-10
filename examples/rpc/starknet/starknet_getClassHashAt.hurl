POST http://0.0.0.0:5050
Content-Type: application/json
{
  "jsonrpc": "2.0",
  "method": "starknet_getClassHashAt",
  "params": [
    "pending",
    "0x295e01fd0ec0e8b5781e0cb0aa9544e4615a3434c95cf1ff4c6b64ca42248ad"
  ],
  "id": 1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" matches /^0x[A-Fa-f0-9]+$/
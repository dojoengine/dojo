POST http://0.0.0.0:5050
Content-Type: application/json
{
  "jsonrpc": "2.0",
  "method": "starknet_getClassHashAt",
  "params": [
    "pending",
    "0x049d36570d4e46f48e99674bd3fcc84644ddd6b96f7c741b1562b82f9e004dc7"
  ],
  "id": 1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" matches /^0x[A-Fa-f0-9]+$/
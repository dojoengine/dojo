POST http://0.0.0.0:5050
Content-Type: application/json
{
  "jsonrpc": "2.0",
  "method": "starknet_getClassHashAt",
  "params": [
    "pending",
    "0x049c7a9c0d35fc7af43743e288974b99bb5393496282ec560180ab77120f44a2"
  ],
  "id": 1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" matches /^0x[A-Fa-f0-9]+$/
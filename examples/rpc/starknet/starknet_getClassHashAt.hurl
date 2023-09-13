POST http://0.0.0.0:5050
Content-Type: application/json
{
  "jsonrpc": "2.0",
  "method": "starknet_getClassHashAt",
  "params": [
    "pending",
    "0x517ececd29116499f4a1b64b094da79ba08dfd54a3edaa316134c41f8160973"
  ],
  "id": 1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" matches /^0x[A-Fa-f0-9]+$/
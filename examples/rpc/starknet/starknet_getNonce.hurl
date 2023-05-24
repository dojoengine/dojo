POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_getNonce",
    "params": [
        "latest",
        "0x03fc02daad5e6fbe732a214d0e44cc7029701004c679dd18f0cc1b8e5276a306"
    ],
    "id":1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" matches /^[0-9]+$/

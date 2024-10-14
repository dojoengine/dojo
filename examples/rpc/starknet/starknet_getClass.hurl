POST http://0.0.0.0:5050
Content-Type: application/json
{
    "jsonrpc": "2.0",
    "method": "starknet_getClass",
    "params": [
        "latest",
        "0x07dc7899aa655b0aae51eadff6d801a58e97dd99cf4666ee59e704249e51adf2"
    ],
    "id":1
}

HTTP 200
[Asserts]
jsonpath "$.error" not exists
jsonpath "$.result" exists
